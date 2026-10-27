//! Sector divisions evidence: entry, one strategy per division, division-wide retooling,
//! dominance pricing and location factors. Expected values are worked by hand from
//! crates/cw-sim/src/economy/sectors.rs and the specializations in
//! content/geography/regions.json.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent, apply};
use cw_sim::economy::{sectors, seed};
use cw_sim::state::{FacilityStatus, ScenarioKind, ScenarioMode, World};

fn world() -> (World, Dataset) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(&dataset, 902, ScenarioKind::Founder, ScenarioMode::Sandbox);
    (world, dataset)
}

fn region(dataset: &Dataset, code: &str) -> u8 {
    dataset.region_index[code]
}

fn fund(world: &mut World, firm: Handle, cents: i64) {
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test funding")
            .credit(Account::ShareCapital, cents, "test funding"),
    );
}

fn build(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    recipe: u16,
) -> Result<(), String> {
    let envelope = CommandEnvelope {
        command_id_high: 7,
        command_id_low: world.revision + 1,
        actor: firm.0,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent: CommandIntent::BuildFacility {
            firm,
            region,
            recipe,
            blocks: 1,
        },
    };
    apply(world, dataset, &envelope)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn dominance_multiplier_follows_share_and_rival_count() {
    assert_eq!(sectors::dominance_multiplier_bps(5_000, 0), 10_000);
    assert_eq!(sectors::dominance_multiplier_bps(3_000, 4), 10_000);
    // 100 percent share, no rivals: 1 + 2 x 1 x 0.35.
    assert_eq!(sectors::dominance_multiplier_bps(10_000, 0), 17_000);
    // 75 percent share: excess 0.5, ramp 0.25. Four rivals pay all of it, two pay 0.675.
    assert_eq!(sectors::dominance_multiplier_bps(7_500, 4), 15_000);
    assert_eq!(sectors::dominance_multiplier_bps(7_500, 9), 15_000);
    assert_eq!(sectors::dominance_multiplier_bps(7_500, 2), 13_375);
}

#[test]
fn location_factors_follow_home_and_state_specialization() {
    let dataset = Dataset::load().unwrap();
    let central_ohio = region(&dataset, "OH-CEN");
    // Home region, a sector Ohio does not specialize in: the calibration baseline.
    assert_eq!(
        sectors::location_factor_bps(&dataset, central_ohio, central_ohio, "retail"),
        10_000
    );
    // Home region, Ohio's secondary specialization (healthcare): 1.5 percent fewer inputs.
    assert_eq!(
        sectors::location_factor_bps(&dataset, central_ohio, central_ohio, "healthcare"),
        9_850
    );
    // Another Ohio region, Ohio's primary specialization (manufacturing): 10250 x 0.97.
    assert_eq!(
        sectors::location_factor_bps(
            &dataset,
            central_ohio,
            region(&dataset, "OH-NE"),
            "manufacturing"
        ),
        9_942
    );
    // Another state, West Virginia's primary specialization (extraction): 10500 x 0.97.
    assert_eq!(
        sectors::location_factor_bps(
            &dataset,
            central_ohio,
            region(&dataset, "WV-SOU"),
            "extraction"
        ),
        10_185
    );
    // Another state, no specialization: 5 percent more inputs.
    assert_eq!(
        sectors::location_factor_bps(&dataset, central_ohio, region(&dataset, "NY-NYC"), "retail"),
        10_500
    );
}

#[test]
fn a_division_runs_one_strategy_and_retools_together() {
    let (mut world, dataset) = world();
    // A farm firm with one plant: that plant is its agriculture division in its region.
    let (firm, plant) = world
        .firms
        .iter()
        .find_map(|(handle, firm)| {
            let plant = *firm.facilities.first()?;
            let recipe = world.facilities.get(plant)?.recipe;
            (dataset.recipes[recipe as usize].industry == "agriculture").then_some((handle, plant))
        })
        .expect("a seeded farm");
    let home = world.facilities.get(plant).unwrap().region;
    let current = world.facilities.get(plant).unwrap().recipe;
    let other = dataset
        .recipes
        .iter()
        .position(|r| r.industry == "agriculture" && r.id != dataset.recipes[current as usize].id)
        .unwrap() as u16;
    fund(&mut world, firm, 50_000_000_000);
    // The seeded firm may already run other divisions of its own; count them.
    let divisions_before = sectors::divisions_of(&world, &dataset, firm).len();

    // A second plant of the same strategy joins the division without an entry fee.
    let quote = sectors::build_cost(&world, &dataset, firm, home, current, 1);
    assert!(!quote.opens_division);
    assert_eq!(quote.entry_fee_cents, 0);
    build(&mut world, &dataset, firm, home, current).expect("same strategy builds");

    // A different strategy in the same division is refused until the division retools.
    let refused = build(&mut world, &dataset, firm, home, other).unwrap_err();
    assert!(refused.contains("division_strategy"), "{refused}");

    // In another region the same firm opens a new division with that strategy.
    let away = (0..dataset.regions.len() as u8)
        .find(|r| *r != home)
        .unwrap();
    let quote = sectors::build_cost(&world, &dataset, firm, away, other, 1);
    assert!(quote.opens_division);
    assert_eq!(quote.entry_fee_cents, sectors::ENTRY_FEE_CENTS);
    assert_eq!(
        quote.capex_cents,
        dataset.recipes[other as usize].capex_cents / 2
    );
    build(&mut world, &dataset, firm, away, other).expect("a new division opens");

    // Finish construction so the home division is idle, then retool it: both plants move.
    let industry = sectors::recipe_industry(&dataset, current);
    let home_plants = sectors::division_plants(&world, &dataset, firm, home, industry);
    assert_eq!(home_plants.len(), 2);
    for handle in &home_plants {
        world.facilities.get_mut(*handle).unwrap().status = FacilityStatus::Operating;
    }
    let envelope = CommandEnvelope {
        command_id_high: 8,
        command_id_low: 1,
        actor: firm.0,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent: CommandIntent::RetoolFacility {
            facility: home_plants[0],
            recipe: other,
        },
    };
    apply(&mut world, &dataset, &envelope).expect("division retools");
    for handle in &home_plants {
        let plant = world.facilities.get(*handle).unwrap();
        assert_eq!(plant.recipe, other, "every plant in the division retools");
        assert!(matches!(plant.status, FacilityStatus::Retooling { .. }));
    }
    assert_eq!(
        sectors::divisions_of(&world, &dataset, firm).len(),
        divisions_before + 1
    );
}

/// Cash across every firm's book plus the outside media and gateway accounts.
fn firm_cash(world: &World) -> i64 {
    world.firms.iter().map(|(_, f)| f.book.cash()).sum::<i64>()
        + world.outside.gateway_book.cash()
        + world.outside.legal_services_book.cash()
}

#[test]
fn a_market_raid_prices_its_stake_and_conserves_blocks_and_money() {
    let (mut world, dataset) = world();
    let (attacker, plant) = world
        .firms
        .iter()
        .find_map(|(handle, firm)| {
            let plant = *firm.facilities.first()?;
            let recipe = world.facilities.get(plant)?.recipe;
            (dataset.recipes[recipe as usize].industry == "agriculture").then_some((handle, plant))
        })
        .expect("a seeded farm");
    let region = world.facilities.get(plant).unwrap().region;
    let recipe = world.facilities.get(plant).unwrap().recipe;
    let industry = sectors::recipe_industry(&dataset, recipe);
    // A rival with no farm here gets a five-block farm division in the same cell.
    let defender = world
        .firms
        .iter()
        .find(|(handle, firm)| {
            *handle != attacker
                && firm.operative()
                && !cw_sim::economy::ownership::same_group(&world, *handle, attacker)
        })
        .map(|(handle, _)| handle)
        .unwrap();
    let mut rival_farm = world.facilities.get(plant).unwrap().clone();
    rival_farm.firm = defender;
    rival_farm.blocks = 5;
    rival_farm.capex_cents = 10_000_000;
    rival_farm.accumulated_depreciation = 0;
    let rival_plant = world.facilities.insert(rival_farm);
    world
        .firms
        .get_mut(defender)
        .unwrap()
        .facilities
        .push(rival_plant);
    let _ = world.firms.get_mut(defender).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::FixedAssets, 10_000_000, "test plant")
            .credit(Account::ShareCapital, 10_000_000, "test plant"),
    );
    for firm in [attacker, defender] {
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .policies
            .marketing_budget_cents = 0;
    }
    fund(&mut world, attacker, 10_000_000_000);

    // Defence floor: five percent of a quarter's output value of the five-block division.
    let template = &dataset.recipes[recipe as usize];
    let price = dataset.reference_price(dataset.product_index[template.output.as_str()]);
    let defence = template.weekly_output_lots * 5 * price * 13 * 500 / 10_000;
    let quote = sectors::raid_quote(
        &world,
        &dataset,
        attacker,
        defender,
        region,
        industry,
        defence * 2,
    );
    assert_eq!(quote.defence_cents, defence);
    // Two-to-one: a quarter of the division is at stake, one block of five; the base chance is
    // two thirds, scaled down by the stake (one fifth of the division costs 17 percent of it).
    assert_eq!(quote.blocks_at_stake, 1);
    assert_eq!(quote.stake_bps, 2_000);
    assert_eq!(quote.chance_bps, 5_532);
    assert_eq!(
        quote.surcharge_cents, 0,
        "a first raid on a rival carries no surcharge"
    );
    assert_eq!(quote.price_cents, 10_000_000 / 5 / 2);

    let blocks = |world: &World, firm: Handle| -> i64 {
        sectors::division_plants(world, &dataset, firm, region, industry)
            .iter()
            .map(|h| world.facilities.get(*h).unwrap().blocks as i64)
            .sum()
    };
    // A division inside its opening grace cannot be raided; once the weeks pass it can.
    let refused = sectors::raid(
        &mut world,
        &dataset,
        attacker,
        defender,
        region,
        industry,
        defence * 2,
    );
    assert!(matches!(
        refused,
        Err(cw_core::error::DomainError::WrongPhase {
            actual: "opening_grace",
            ..
        })
    ));
    world.tick.0 += sectors::RAID_GRACE_WEEKS;
    let total_blocks = blocks(&world, attacker) + blocks(&world, defender);
    let mut successes = 0;
    for attempt in 0..30 {
        let money = firm_cash(&world);
        let record = sectors::raid(
            &mut world,
            &dataset,
            attacker,
            defender,
            region,
            industry,
            defence * 2,
        )
        .expect("raid resolves");
        assert_eq!(
            firm_cash(&world),
            money,
            "a raid moves money, never mints it"
        );
        assert_eq!(
            blocks(&world, attacker) + blocks(&world, defender),
            total_blocks
        );
        if record.success {
            successes += 1;
            assert!(record.blocks >= 1);
        }
        // No cooldown: raiding the same rival again is allowed, but it costs more each time.
        let again = sectors::raid_quote(
            &world,
            &dataset,
            attacker,
            defender,
            region,
            industry,
            defence * 2,
        );
        assert_eq!(
            again.surcharge_cents,
            defence * 2 * 4_000 / 10_000,
            "a second raid in the quarter pays forty percent of its budget again"
        );
        world.tick.0 += 13;
        if blocks(&world, defender) < 2 {
            break;
        }
        let _ = attempt;
    }
    assert!(successes > 0, "a two-to-one raid lands within thirty tries");
    assert!(world.raids.len() >= successes);
}
