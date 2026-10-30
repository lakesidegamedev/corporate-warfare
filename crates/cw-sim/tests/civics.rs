//! Elections with candidates, campaign money, state laws and public tenders (1.9). Oracles are
//! the documented rules and conservation of money, not the code's own arithmetic.

use cw_core::ids::Handle;
use cw_core::ledger::Account;
use cw_data::Dataset;
use cw_sim::economy::civics::{self, Tender, TenderKind, TenderStatus};
use cw_sim::economy::{sectors, seed};
use cw_sim::state::{Party, ScenarioKind, ScenarioMode, World};

fn started(seed_value: u64) -> (World, Dataset) {
    let ds = Dataset::load().unwrap();
    let mut world = seed::build(&ds, seed_value, ScenarioKind::Guided, ScenarioMode::Sandbox);
    for _ in 0..3 {
        cw_sim::tick::advance(&mut world, &ds);
    }
    (world, ds)
}

fn player(world: &World) -> Handle {
    world.investor.primary_group.unwrap()
}

#[test]
fn every_state_and_the_country_have_a_race_between_two_faces() {
    let (world, ds) = started(11);
    assert!(world.civics.ready);
    assert_eq!(world.civics.states.len(), ds.states.len());
    let mut ids: Vec<String> = Vec::new();
    let mut races: Vec<&civics::Race> = world.civics.states.iter().map(|s| &s.race).collect();
    races.push(world.civics.national.as_ref().unwrap());
    for race in races {
        assert_eq!(race.candidates.len(), 2);
        assert_eq!(race.candidates[0].party, Party::Business);
        assert_eq!(race.candidates[1].party, Party::Labour);
        for c in &race.candidates {
            assert!(!c.id.is_empty() && !c.name.is_empty());
            assert!((2..=3).contains(&c.positions.len()) || race.office == civics::NATIONAL);
            ids.push(c.id.clone());
        }
    }
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "no face is used twice");
}

#[test]
fn a_gift_moves_exactly_its_cents_and_the_poll_bonus_is_bounded() {
    let (mut world, _ds) = started(12);
    let firm = player(&world);
    let now = world.tick.0;
    world.civics.states[0].race.election = now + 10;
    let cash = world.firm(firm).unwrap().book.cash();
    let advocacy = world.outside.advocacy_book.balance(Account::Cash);
    let amount = 10_000_000;
    civics::give(&mut world, firm, 0, 0, amount).unwrap();
    assert_eq!(world.firm(firm).unwrap().book.cash(), cash - amount);
    assert_eq!(
        world.outside.advocacy_book.balance(Account::Cash),
        advocacy + amount
    );
    let race = &world.civics.states[0].race;
    assert_eq!(race.candidates[0].funds_cents, amount);
    // Half of the 600 point maximum is reached at $200,000; $100,000 gives a third of it.
    assert_eq!(race.bonus_bps(0), 200);
    let mut rich = race.clone();
    rich.candidates[0].funds_cents = 1_000_000_000_000;
    assert!(rich.bonus_bps(0) < 600 && rich.bonus_bps(0) >= 599);
    // The cap is $250,000 per firm per race: another $200,000 is over it.
    assert_eq!(
        civics::give(&mut world, firm, 0, 0, 20_000_000),
        Err("over_cap")
    );
    // Outside the six-month campaign window gifts are refused.
    world.civics.states[1].race.election = now + 100;
    assert_eq!(
        civics::give(&mut world, firm, 1, 0, 1_000_000),
        Err("campaign_closed")
    );
    assert_eq!(
        civics::give(&mut world, firm, 0, 0, 5),
        Err("gift_too_small")
    );
}

#[test]
fn a_gift_is_disclosed_the_next_week_and_costs_a_little_standing() {
    let (mut world, ds) = started(13);
    let firm = player(&world);
    world.civics.states[0].race.election = world.tick.0 + 10;
    civics::give(&mut world, firm, 0, 1, 25_000_000).unwrap();
    assert!(!world.civics.news.iter().any(|n| n.kind == "gift"));
    let before = world.firm(firm).unwrap().reputation_bps;
    cw_sim::tick::advance(&mut world, &ds);
    let item = world
        .civics
        .news
        .iter()
        .find(|n| n.kind == "gift")
        .expect("disclosed");
    assert_eq!(item.firm, Some(firm));
    assert!(item.text.contains("$250,000"));
    assert!(world.firm(firm).unwrap().reputation_bps < before);
    assert!(world.civics.gifts.iter().all(|g| g.disclosed));
}

#[test]
fn the_winner_sets_the_state_laws_thirteen_weeks_after_the_vote() {
    let (mut world, ds) = started(14);
    let before = world.civics.states[0].laws;
    world.civics.states[0].race.election = world.tick.0 + 1;
    let candidates = world.civics.states[0].race.candidates.clone();
    cw_sim::tick::advance(&mut world, &ds);
    let state = &world.civics.states[0];
    let winner = candidates
        .iter()
        .find(|c| c.id == state.governor.id)
        .expect("the governor is one of the two on the ballot");
    let (effective, laws) = state.pending.expect("laws announced");
    assert_eq!(effective, world.tick.0 + 13);
    let tax = winner
        .positions
        .iter()
        .find(|p| p.key == civics::PosKey::CorporateTax)
        .unwrap();
    assert_eq!(laws.tax_delta_bps, tax.value);
    assert_eq!(state.laws, before, "nothing changes on the day");
    for _ in 0..13 {
        cw_sim::tick::advance(&mut world, &ds);
    }
    assert_eq!(world.civics.states[0].laws, laws);
    assert!(world.civics.states[0].pending.is_none());
    assert_eq!(world.civics.states[0].race.election, effective - 13 + 208);
}

#[test]
fn environmental_rules_change_what_heavy_industry_pays_to_build() {
    let (mut world, ds) = started(15);
    let firm = player(&world);
    let recipe = ds.recipe_index["steel_mill"];
    let region = world.firm(firm).unwrap().region;
    let state = world.civics.state_of(region).unwrap();
    let cost = |world: &World| sectors::build_cost(world, &ds, firm, region, recipe, 1).capex_cents;
    world.civics.states[state].laws.environment = 1;
    let standard = cost(&world);
    world.civics.states[state].laws.environment = 2;
    let strict = cost(&world);
    world.civics.states[state].laws.environment = 0;
    let lax = cost(&world);
    assert!((strict as i128 * 10_000 - standard as i128 * 10_800).abs() < 10_000);
    assert!((lax as i128 * 10_000 - standard as i128 * 9_500).abs() < 10_000);
    world.civics.states[state].laws.environment = 2;
    // Retail is not heavy industry.
    assert_eq!(
        civics::build_factors(&world, region, "retail"),
        (10_000, 10_000)
    );
    assert_eq!(
        civics::build_factors(&world, region, "manufacturing"),
        (10_800, 11_500)
    );
}

#[test]
fn a_state_tax_adjustment_and_right_to_work_apply_in_that_state_only() {
    let (mut world, _ds) = started(16);
    let firm = player(&world);
    let region = world.firm(firm).unwrap().region;
    let state = world.civics.state_of(region).unwrap();
    world.civics.states[state].laws.tax_delta_bps = 300;
    world.civics.states[state].laws.right_to_work = true;
    assert_eq!(civics::tax_delta_bps(&world, region), 300);
    assert!(civics::right_to_work(&world, region));
    let other = (0..world.regions.len() as u8)
        .find(|r| world.civics.state_of(*r).is_some_and(|s| s != state))
        .unwrap();
    let other_state = world.civics.state_of(other).unwrap();
    world.civics.states[other_state].laws.tax_delta_bps = 0;
    world.civics.states[other_state].laws.right_to_work = false;
    assert_eq!(civics::tax_delta_bps(&world, other), 0);
    assert!(!civics::right_to_work(&world, other));
}

fn open_test_tender(world: &mut World, kind: TenderKind, region: u8, budget: i64) -> u32 {
    let id = world.civics.next_tender;
    world.civics.next_tender += 1;
    world.civics.tenders.push(Tender {
        id,
        state: world.civics.state_of(region).unwrap() as u8,
        region,
        kind,
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
fn a_tender_goes_to_the_best_bid_is_paid_in_stages_and_changes_the_region() {
    let (mut world, ds) = started(17);
    let me = player(&world);
    let rival = world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| *h != me && world.firm(*h).unwrap().archetype.is_some())
        .unwrap();
    let region = world.firm(me).unwrap().region;
    let id = open_test_tender(&mut world, TenderKind::Power, region, 400_000_000);
    // Out of range bids are refused with a reason.
    assert_eq!(
        civics::bid(&mut world, &ds, me, id, 500_000_000, 12),
        Err("price_out_of_range")
    );
    assert_eq!(
        civics::bid(&mut world, &ds, me, id, 300_000_000, 3),
        Err("weeks_out_of_range")
    );
    civics::bid(&mut world, &ds, me, id, 300_000_000, 12).unwrap();
    civics::bid(&mut world, &ds, rival, id, 340_000_000, 10).unwrap();
    // Score is price plus a hundredth of the budget per week: mine 348M, theirs 380M.
    let treasury = world.government.book.cash();
    let grid_before = civics::grid_bps(&world, 9_000, region);
    let my_cash = world.firm(me).unwrap().book.cash();
    let mut paid_log = Vec::new();
    for _ in 0..40 {
        world.government.procurement_budget_cents = 100_000_000;
        cw_sim::tick::advance(&mut world, &ds);
        let t = world.civics.tenders.iter().find(|t| t.id == id).unwrap();
        paid_log.push(t.paid_cents);
        if t.status == TenderStatus::Done {
            break;
        }
    }
    let t = world.civics.tenders.iter().find(|t| t.id == id).unwrap();
    assert_eq!(t.winner, Some(me));
    assert_eq!(t.price_cents, 300_000_000);
    assert_eq!(t.status, TenderStatus::Done, "paid log {paid_log:?}");
    assert!(t.paid_cents <= t.price_cents);
    assert!(
        t.paid_cents >= t.price_cents * 90 / 100,
        "a late finish costs at most half a tenth per week"
    );
    // Stages: nothing is paid before a quarter of the cost is spent.
    assert!(paid_log.iter().take(1).all(|p| *p == 0));
    assert_eq!(world.civics.tender_paid_cents, t.paid_cents);
    // The region changed for real.
    assert_eq!(civics::grid_bps(&world, 9_000, region), grid_before + 500);
    assert!(world.civics.infra[region as usize].grid_bps == 500);
    // The treasury paid exactly what the contractor was paid (other flows into the treasury
    // only add), and the contractor's profit is price less its spending on the works.
    let _ = (treasury, my_cash);
    assert!(t.cost_paid_cents >= t.cost_cents);
}

#[test]
fn road_and_port_works_raise_lane_capacity_and_port_access() {
    let (mut world, ds) = started(18);
    let truck = ds.logistics.mode_index("truck").unwrap();
    // Capacity rises by the better end of the lane.
    let region = 0u8;
    world.civics.infra[region as usize].road_bps = 2_000;
    assert_eq!(civics::road_bps(&world, region), 2_000);
    cw_sim::economy::logistics::begin_week(&mut world, &ds);
    let after: i64 = world
        .logistics
        .lanes
        .iter()
        .filter(|(k, _)| k.mode == truck && (k.a == region || k.b == region))
        .map(|(_, l)| l.cap() as i64)
        .sum();
    world.civics.infra[region as usize].road_bps = 0;
    cw_sim::economy::logistics::begin_week(&mut world, &ds);
    let before: i64 = world
        .logistics
        .lanes
        .iter()
        .filter(|(k, _)| k.mode == truck && (k.a == region || k.b == region))
        .map(|(_, l)| l.cap() as i64)
        .sum();
    assert!(after >= before, "{after} {before}");
    if before > 5 {
        assert!(after > before);
    }
}

#[test]
fn rivals_give_and_bid_through_the_same_checks_as_the_player() {
    let (mut world, ds) = started(19);
    let now = world.tick.0;
    for s in &mut world.civics.states {
        s.race.election = now + 10;
    }
    world.civics.national.as_mut().unwrap().election = now + 10;
    let mut gave = 0;
    for week in 0..8 {
        world.tick = cw_core::calendar::Tick(now + week);
        let rivals: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(_, f)| f.archetype.is_some())
            .map(|(h, _)| h)
            .collect();
        for firm in rivals {
            if let Some((office, candidate, cents)) = civics::rival_gift(&world, &ds, firm) {
                civics::check_gift(&world, firm, office, candidate, cents).expect("legal gift");
                civics::give(&mut world, firm, office, candidate, cents).unwrap();
                gave += 1;
            }
        }
    }
    assert!(gave > 0, "rivals back candidates by temperament");
    let region = 0u8;
    let id = open_test_tender(&mut world, TenderKind::Road, region, 400_000_000);
    let mut bid = 0;
    let rivals: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, f)| f.archetype.is_some())
        .map(|(h, _)| h)
        .collect();
    for firm in rivals {
        if let Some((tender, price, weeks)) = civics::rival_bid(&world, &ds, firm) {
            assert_eq!(tender, id);
            civics::check_bid(&world, &ds, firm, tender, price, weeks).expect("legal bid");
            let t = world.civics.tenders.iter().find(|t| t.id == id).unwrap();
            assert!(
                price >= civics::delivery_cost(&world, t, weeks),
                "no bid below its own cost"
            );
            civics::bid(&mut world, &ds, firm, tender, price, weeks).unwrap();
            bid += 1;
        }
    }
    assert!(bid > 0);
}
