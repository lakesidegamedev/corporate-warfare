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
    assert_eq!(sectors::divisions_of(&world, &dataset, firm).len(), 2);
}
