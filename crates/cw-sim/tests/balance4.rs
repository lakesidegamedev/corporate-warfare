//! Balance pass 4 (1.9): fair tenders, a treasury that answers to its cash, opening share prices
//! that sit where the shares trade a quarter later, and the player's own chief. Oracles are hand
//! figures and conservation, not the code's own arithmetic.

use cw_data::Dataset;
use cw_sim::economy::civics::{self, Tender, TenderKind, TenderStatus};
use cw_sim::economy::{fiscal, seed};
use cw_sim::state::{PlayerChief, ScenarioKind, ScenarioMode, World};

fn world_of(seed_value: u64) -> (World, Dataset) {
    let ds = Dataset::load().unwrap();
    let world = seed::build(&ds, seed_value, ScenarioKind::Guided, ScenarioMode::Sandbox);
    (world, ds)
}

#[test]
fn the_labour_tax_and_the_budgets_follow_the_treasury_by_hand_figures() {
    // At the reserve: twenty percent and the plain budgets.
    assert_eq!(fiscal::labour_tax_bps(10_000), 2_000);
    assert_eq!(fiscal::spending_factor_bps(10_000), 10_000);
    // A point of tax for every 1,200 basis points of stance over the reserve (an eighth).
    assert_eq!(fiscal::labour_tax_bps(11_200), 1_900);
    assert_eq!(fiscal::labour_tax_bps(13_600), 1_700);
    // Far over it the tax stops at eight percent.
    assert_eq!(fiscal::labour_tax_bps(1_000_000), 800);
    // Half the excess is spent on top, to at most 130 percent.
    assert_eq!(fiscal::spending_factor_bps(12_000), 11_000);
    assert_eq!(fiscal::spending_factor_bps(30_000), 13_000);
    // Under the reserve: a point more tax for every 2,500 short (to 22 percent), budgets cut in
    // proportion down to half.
    assert_eq!(fiscal::labour_tax_bps(7_500), 2_100);
    assert_eq!(fiscal::labour_tax_bps(0), 2_200);
    assert_eq!(fiscal::spending_factor_bps(8_000), 8_000);
    assert_eq!(fiscal::spending_factor_bps(2_000), 5_000);
}

#[test]
fn stance_is_cash_against_thirteen_weeks_of_receipts() {
    let (mut world, _) = world_of(41);
    // The treasury opens with thirteen weeks of receipts, so it sits at its reserve.
    let mean = fiscal::mean_receipts(&world);
    let cash = world.government.book.cash();
    assert!(mean > 0 && cash > 0);
    let stance = fiscal::stance_bps(&world);
    assert_eq!(stance, (cash as i128 * 10_000 / (mean as i128 * 13)) as i64);
    assert!((9_000..=11_000).contains(&stance), "stance {stance}");
    // Twice the receipts for the same cash is half the reserve.
    world.government.tax_receipts_history = vec![mean * 2; 13];
    assert_eq!(
        fiscal::stance_bps(&world),
        (cash as i128 * 10_000 / (mean as i128 * 2 * 13)) as i64
    );
}

#[test]
fn tenders_open_faster_when_flush_and_stop_when_the_treasury_is_thin() {
    assert_eq!(civics::tender_interval(10_000), 13);
    assert_eq!(civics::tender_interval(15_000), 6);
    assert_eq!(civics::tender_interval(40_000), 6);
    assert_eq!(civics::tender_interval(4_999), u32::MAX);
}

fn open_tender(world: &mut World, region: u8, budget: i64) -> u32 {
    let id = world.civics.next_tender;
    world.civics.next_tender += 1;
    world.civics.tenders.push(Tender {
        id,
        state: world.civics.state_of(region).unwrap() as u8,
        region,
        kind: TenderKind::Road,
        budget_cents: budget,
        opened: world.tick.0,
        closes: world.tick.0 + 2,
        min_weeks: 8,
        max_weeks: 26,
        bids: Vec::new(),
        status: TenderStatus::Open,
        winner: None,
        price_cents: 0,
        promised_weeks: 0,
        started: 0,
        cost_cents: 0,
        cost_paid_cents: 0,
        paid_cents: 0,
        finished: 0,
    });
    id
}

#[test]
fn works_cost_more_than_the_estimate_by_a_hidden_figure_and_pay_as_they_progress() {
    let ds = Dataset::load().unwrap();
    let mut world = seed::build(&ds, 17, ScenarioKind::Guided, ScenarioMode::Sandbox);
    for _ in 0..3 {
        cw_sim::tick::advance(&mut world, &ds);
    }
    let me = world.investor.primary_group.unwrap();
    let region = world.firm(me).unwrap().region;
    let id = open_tender(&mut world, region, 400_000_000);
    let estimate = {
        let t = world.civics.tenders.iter().find(|t| t.id == id).unwrap();
        civics::delivery_cost(&world, t, 12)
    };
    civics::bid(&mut world, &ds, me, id, 390_000_000, 12).unwrap();
    let overrun = civics::overrun_bps(&world, id);
    assert!((0..=800).contains(&overrun), "overrun {overrun} bps");
    // The same tender always runs over by the same figure.
    assert_eq!(overrun, civics::overrun_bps(&world, id));
    let mut checked = 0;
    for _ in 0..30 {
        world.government.procurement_budget_cents = 400_000_000;
        cw_sim::tick::advance(&mut world, &ds);
        let t = world.civics.tenders.iter().find(|t| t.id == id).unwrap();
        if t.status == TenderStatus::Building {
            // The bill the winner books is the estimate plus the overrun.
            assert_eq!(t.cost_cents, estimate * (10_000 + overrun) / 10_000);
            // The state pays in step with progress, not in quarters.
            let progress = (t.cost_paid_cents * 10_000 / t.cost_cents).min(10_000);
            assert_eq!(t.paid_cents, t.price_cents * progress / 10_000);
            checked += 1;
        }
    }
    assert!(checked >= 5, "the works ran for weeks: {checked}");
}

#[test]
fn every_rival_bid_is_within_the_ceiling_and_sits_near_cost() {
    let ds = Dataset::load().unwrap();
    let mut world = seed::build(&ds, 23, ScenarioKind::Guided, ScenarioMode::Sandbox);
    for _ in 0..3 {
        cw_sim::tick::advance(&mut world, &ds);
    }
    let region = world
        .regions
        .iter()
        .position(|r| r.land_index_bps > 0)
        .unwrap() as u8;
    let id = open_tender(&mut world, region, 600_000_000);
    let handles: Vec<_> = world.firms.iter().map(|(h, _)| h).collect();
    let mut bids = 0;
    for h in handles {
        if let Some((tender, price, weeks)) = civics::rival_bid(&world, &ds, h) {
            assert_eq!(tender, id);
            let t = world.civics.tenders.iter().find(|t| t.id == id).unwrap();
            let cost = civics::delivery_cost(&world, t, weeks);
            assert!(price <= t.budget_cents);
            // Between 5 percent under (a low reading of the cost, a low margin) and 15 percent
            // over: a normal margin, not the old 10 to 22.
            assert!(price * 100 >= cost * 95, "price {price} cost {cost}");
            assert!(price * 100 <= cost * 115, "price {price} cost {cost}");
            bids += 1;
        }
    }
    assert!(bids >= 3, "several firms want the work: {bids}");
}

#[test]
fn the_players_chief_takes_the_kit_when_the_choice_is_blank_or_unknown() {
    let kit = cw_data::cast::extras();
    let first = &kit[0];
    let blank = PlayerChief::choose("", "", "").unwrap();
    assert_eq!(blank.portrait, first.id);
    assert_eq!(blank.name, first.name);
    assert_eq!(blank.temperament, first.temperament.key());
    let unknown = PlayerChief::choose("  Rae  ", "no_such_portrait", "bogus").unwrap();
    assert_eq!(unknown.portrait, first.id);
    assert_eq!(unknown.name, "Rae");
    assert_eq!(unknown.temperament, first.temperament.key());
    let second = &kit[1];
    let picked = PlayerChief::choose("", &second.id, "cautious").unwrap();
    assert_eq!(picked.portrait, second.id);
    assert_eq!(picked.temperament, "cautious");
    let long = "x".repeat(200);
    assert_eq!(
        PlayerChief::choose(&long, "", "")
            .unwrap()
            .name
            .chars()
            .count(),
        PlayerChief::NAME_MAX
    );
    // Control characters are dropped.
    assert_eq!(PlayerChief::choose("A\u{0}b\n", "", "").unwrap().name, "Ab");
}
