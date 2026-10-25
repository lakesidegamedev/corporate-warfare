//! Funded research, saved draws, brand stock/decay and bounded quality (packet P26).
//!
//! The oracles here are the documented rules in `docs/rules/CONTENT-PACING.md`: a squared
//! level threshold, one saved 7500-bps draw at the threshold, half progress retained on
//! failure, a frontier/level ceiling, brand gains bounded at +100 bps a week (capped
//! 7500..15000, decaying 10 a week toward 10000) and quality bounded by level and
//! investment. Nothing here asserts private implementation shapes.

use cw_core::ids::Handle;
use cw_core::ledger::Account;
use cw_data::Dataset;
use cw_sim::economy::{research, seed};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};

fn dataset() -> Dataset {
    Dataset::load().expect("pinned dataset validates")
}

fn seeded(seed_value: u64) -> World {
    seed::build(
        &dataset(),
        seed_value,
        ScenarioKind::Founder,
        ScenarioMode::Sandbox,
    )
}

fn technology_firm(world: &World) -> Handle {
    world
        .firms
        .iter()
        .find(|(_, firm)| firm.industry == research::industry_of("technology").unwrap())
        .map(|(handle, _)| handle)
        .expect("seeded technology firm")
}

fn funded(world: &mut World, firm: Handle, cents: i64) {
    use cw_core::ledger::Txn;
    let mut book = world.firm(firm).expect("firm").book.clone();
    book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test capital")
            .credit(Account::ShareCapital, cents, "test capital"),
    )
    .expect("capital posts");
    world.firms.get_mut(firm).unwrap().book = book;
}

fn assert_books(world: &World) {
    for (_, firm) in world.firms.iter() {
        assert!(firm.book.is_balanced(), "{} balanced", firm.name);
        assert!(firm.book.cash() >= 0, "{} cash non-negative", firm.name);
    }
}

// ---------------------------------------------------------------------------------------
// Thresholds and funding
// ---------------------------------------------------------------------------------------

#[test]
fn research_thresholds_are_squared_and_monotone() {
    assert_eq!(research::threshold(0), 10_000_000);
    assert_eq!(research::threshold(1), 40_000_000);
    assert_eq!(research::threshold(2), 90_000_000);
    assert_eq!(research::threshold(4), 250_000_000);
    for level in 1..research::MAX_TECH_LEVEL {
        assert!(
            research::threshold(level + 1) > research::threshold(level),
            "threshold rises with level {level}"
        );
    }
}

#[test]
fn research_without_a_budget_never_innovates() {
    let dataset = dataset();
    let mut world = seeded(201);
    let firm = technology_firm(&world);
    let level = world.firm(firm).unwrap().tech_level;
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .policies
        .research_budget_cents = 0;
    // Progress already at the threshold is not spent into an upgrade without funding.
    world.firms.get_mut(firm).unwrap().tech_progress_cents = research::threshold(level);
    let cash = world.firm(firm).unwrap().book.cash();
    for _ in 0..5 {
        research::weekly(&mut world);
    }
    assert_eq!(world.firm(firm).unwrap().tech_level, level, "no free level");
    assert_eq!(
        world.firm(firm).unwrap().tech_progress_cents,
        research::threshold(level)
    );
    assert_eq!(world.firm(firm).unwrap().book.cash(), cash, "nothing spent");
    let _ = dataset;
    assert_books(&world);
}

#[test]
fn research_progress_is_funded_and_the_draw_is_saved() {
    let dataset = dataset();
    let mut world = seeded(202);
    let firm = technology_firm(&world);
    let level = world.firm(firm).unwrap().tech_level;
    let budget = 10_000_000;
    funded(&mut world, firm, 500_000_000);
    {
        let entity = world.firms.get_mut(firm).unwrap();
        entity.policies.research_budget_cents = budget;
        entity.tech_progress_cents = research::threshold(level) - budget;
    }
    let cash_before = world.firm(firm).unwrap().book.cash();
    let expense_before = world
        .firm(firm)
        .unwrap()
        .book
        .balance(Account::OperatingExpense);

    research::weekly(&mut world);

    let after = world.firm(firm).unwrap();
    let level_after = after.tech_level;
    let progress_after = after.tech_progress_cents;
    // Spend is real money and is expensed whether the draw succeeds or fails.
    assert_eq!(
        cash_before - after.book.cash(),
        budget,
        "the budget was paid"
    );
    assert_eq!(
        after.book.balance(Account::OperatingExpense) - expense_before,
        budget,
        "research expense is recognised"
    );
    if level_after == level {
        assert_eq!(
            progress_after,
            (research::threshold(level) - budget) / 2,
            "a failed draw retains half the accumulated progress"
        );
    } else {
        assert_eq!(level_after, level + 1, "at most one level per threshold");
        assert_eq!(progress_after, 0, "successful research resets progress");
    }
    assert_books(&world);

    // Reload fairness: the same seed, firm and funding produce the same outcome, so the draw
    // is saved rather than rerolled by restarting.
    let mut first = seeded(202);
    let mut second = seeded(202);
    for candidate in [first.firms.iter_mut(), second.firms.iter_mut()] {
        for (_, entity) in candidate {
            entity.policies.research_budget_cents = 0;
        }
    }
    let target = technology_firm(&first);
    let other = technology_firm(&second);
    assert_eq!(target, other);
    for entity in [&mut first, &mut second] {
        funded(entity, target, 500_000_000);
        let level = entity.firm(target).unwrap().tech_level;
        let row = entity.firms.get_mut(target).unwrap();
        row.policies.research_budget_cents = budget;
        row.tech_progress_cents = research::threshold(level) - budget;
        research::weekly(entity);
    }
    assert_eq!(
        first.firm(target).unwrap().tech_level,
        second.firm(target).unwrap().tech_level
    );
    assert_eq!(
        first.firm(target).unwrap().tech_progress_cents,
        second.firm(target).unwrap().tech_progress_cents
    );
    let _ = dataset;
}

#[test]
fn research_stops_at_the_frontier_and_the_level_ceiling() {
    let dataset = dataset();
    let mut world = seeded(203);
    let firm = technology_firm(&world);
    funded(&mut world, firm, 1_000_000_000);
    // Frontier + 1 is the highest level research can reach.
    world.macro_state.tech_frontier = 2;
    world.firms.get_mut(firm).unwrap().tech_level = 3;
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .policies
        .research_budget_cents = 50_000_000;
    world.firms.get_mut(firm).unwrap().tech_progress_cents = 1_000_000_000;
    let cash_before = world.firm(firm).unwrap().book.cash();
    research::weekly(&mut world);
    assert_eq!(
        world.firm(firm).unwrap().tech_level,
        3,
        "already at the frontier cap"
    );
    assert_eq!(
        world.firm(firm).unwrap().tech_progress_cents,
        1_000_000_000,
        "capped research does not bank progress"
    );
    assert_eq!(
        cash_before - world.firm(firm).unwrap().book.cash(),
        50_000_000,
        "the failed spend still costs money"
    );

    // The absolute ceiling is 20 even with an unbounded frontier.
    world.macro_state.tech_frontier = 200;
    world.firms.get_mut(firm).unwrap().tech_level = research::MAX_TECH_LEVEL;
    world.firms.get_mut(firm).unwrap().tech_progress_cents = 0;
    research::weekly(&mut world);
    assert_eq!(
        world.firm(firm).unwrap().tech_level,
        research::MAX_TECH_LEVEL
    );
    assert_books(&world);
    let _ = dataset;
}

// ---------------------------------------------------------------------------------------
// Brand and quality
// ---------------------------------------------------------------------------------------

#[test]
fn brand_spend_is_capped_by_budget_and_rate() {
    let dataset = dataset();
    let mut world = seeded(204);
    let firm = technology_firm(&world);
    let region = world.firm(firm).unwrap().region;
    let product = dataset.product_code("software").unwrap();
    let baseline = 1_000_000;

    // Spending the whole category baseline would be +10000 bps, but one week is capped at +100.
    let cash_before = world.firm(firm).unwrap().book.cash();
    let gain = research::spend_brand(&mut world, firm, region, product, baseline, baseline);
    assert_eq!(gain, 100, "one week's brand gain is capped at 100 bps");
    assert_eq!(
        world.firm(firm).unwrap().brand_bps[&(region, product)],
        10_100
    );
    assert_eq!(
        cash_before - world.firm(firm).unwrap().book.cash(),
        baseline,
        "brand spend is paid in full"
    );

    // Brand stock is capped at 15000 bps however often it is bought.
    for _ in 0..80 {
        research::spend_brand(&mut world, firm, region, product, baseline / 100, baseline);
    }
    assert_eq!(
        world.firm(firm).unwrap().brand_bps[&(region, product)],
        research::BRAND_CAP
    );
    assert_books(&world);

    // Spending cannot exceed available cash: the payment stops at zero cash and the gain is
    // computed from what was actually funded.
    let cash = world.firm(firm).unwrap().book.cash();
    let gain = research::spend_brand(&mut world, firm, region, product, i64::MAX / 2, baseline);
    assert_eq!(gain, 100);
    assert_eq!(world.firm(firm).unwrap().book.cash(), 0, "no overdraft");
    assert!(cash > 0);
    assert_books(&world);
    let _ = dataset;
}

#[test]
fn brand_decays_ten_a_week_toward_neutral_inside_bounds() {
    let dataset = dataset();
    let mut world = seeded(205);
    let firm = technology_firm(&world);
    let region = world.firm(firm).unwrap().region;
    let product = dataset.product_code("software").unwrap();
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .brand_bps
        .insert((region, product), research::BRAND_CAP);
    for expected in [14_990, 14_980, 14_970] {
        research::weekly(&mut world);
        assert_eq!(
            world.firm(firm).unwrap().brand_bps[&(region, product)],
            expected,
            "brand decays toward the neutral baseline"
        );
    }
    // A below-neutral stock also decays toward 10000 without crossing the floor.
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .brand_bps
        .insert((region, product), research::BRAND_FLOOR);
    for _ in 0..10 {
        research::weekly(&mut world);
        let value = world.firm(firm).unwrap().brand_bps[&(region, product)];
        assert!((research::BRAND_FLOOR..=research::BRAND_CAP).contains(&value));
    }
    assert_eq!(
        world.firm(firm).unwrap().brand_bps[&(region, product)],
        research::BRAND_FLOOR + 100
    );
    assert_books(&world);
    let _ = dataset;
}

#[test]
fn quality_and_efficiency_are_bounded() {
    assert_eq!(research::quality(0, 0), 10_000);
    assert_eq!(
        research::quality(1, 0),
        10_200,
        "one tech level is worth 200 bps of quality"
    );
    assert_eq!(
        research::quality(5, 1_000),
        12_000,
        "quality investment adds at most 1000 bps"
    );
    for level in 0..=research::MAX_TECH_LEVEL {
        for investment in [0, 500, 1_000, 5_000] {
            let value = research::quality(level, investment);
            assert!(
                (10_000..=20_000).contains(&value),
                "quality {value} stays inside the declared bounds"
            );
        }
    }
    assert_eq!(research::intensity_bps(0), 10_000);
    assert_eq!(research::intensity_bps(20), 6_000);
    assert_eq!(
        research::intensity_bps(255),
        6_000,
        "intensity floors at 60 percent of baseline, never below"
    );
    for level in 1..=research::MAX_TECH_LEVEL {
        assert!(research::intensity_bps(level) <= research::intensity_bps(level - 1));
    }
}
