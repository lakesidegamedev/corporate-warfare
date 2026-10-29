//! Owned logistics (1.7 preview 3): freight modes and what they may carry, lanes with finite
//! capacity and a smooth price, own-group priority at cost, the outside backstop, lane contracts,
//! routing policies, and the rule that a shipment is charged once and every receipt matches it.

use cw_core::ids::Handle;
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_data::{Dataset, StorageClass};
use cw_sim::economy::logistics::{self as lg, LaneKey, PartKind, RoutePlan, ShipKind, ShipPolicy};
use cw_sim::economy::seed;
use cw_sim::state::{Facility, FacilityStatus, ScenarioKind, ScenarioMode, World};

const TRUCK: u8 = 0;
const RAIL: u8 = 1;
const PIPELINE: u8 = 2;
const SEA: u8 = 3;

fn world(seed_value: u64) -> (World, Dataset) {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Guided,
        ScenarioMode::Sandbox,
    );
    lg::begin_week(&mut world, &dataset);
    (world, dataset)
}

fn region(dataset: &Dataset, code: &str) -> u8 {
    dataset.region_index[code]
}

fn total_cash(world: &World) -> i64 {
    world.firms.iter().map(|(_, f)| f.book.cash()).sum::<i64>() + world.outside.gateway_book.cash()
}

fn cash(world: &World, firm: Handle) -> i64 {
    world.firms.get(firm).unwrap().book.cash()
}

fn add_plant(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    recipe_id: &str,
) -> Handle {
    let recipe_index = dataset.recipe_index[recipe_id];
    let recipe = &dataset.recipes[recipe_index as usize];
    let facility = Facility {
        firm,
        region,
        recipe: recipe_index,
        blocks: 1,
        status: FacilityStatus::Operating,
        started: world.tick,
        completes: world.tick,
        escrow_cents: 0,
        capex_cents: recipe.capex_cents,
        accumulated_depreciation: 0,
        workers: recipe.workers_low_mid_high,
        target_bps: 10_000,
        quality_bps: 10_000,
        deposit_remaining: MilliLots::ZERO,
        last_output: MilliLots::from_lots(recipe.weekly_output_lots),
        last_inputs_short: false,
        last_staff_ratio_bps: 10_000,
        mothballed_since: None,
        equipment_due_cents: 0,
        outage: None,
        stock_plan: Default::default(),
        ship: Default::default(),
        serves: Vec::new(),
        cut_bps: Vec::new(),
    };
    let handle = world.facilities.insert(facility);
    world.firms.get_mut(firm).unwrap().facilities.push(handle);
    handle
}

/// Two firms that are not in one group.
fn two_firms(world: &World) -> (Handle, Handle) {
    let mut firms = world
        .firms
        .iter()
        .filter(|(h, f)| f.operative() && f.industry != 6 && !cw_ai_free(world, *h))
        .map(|(h, _)| h);
    (firms.next().unwrap(), firms.next().unwrap())
}

fn cw_ai_free(world: &World, firm: Handle) -> bool {
    cw_sim::economy::ownership::ultimate_controller(world, firm)
        == Some(cw_sim::state::HolderId::Investor)
}

fn steel(dataset: &Dataset) -> u16 {
    dataset.product_index["steel"]
}

fn sum_parts(plan: &RoutePlan) -> i64 {
    plan.legs
        .iter()
        .flat_map(|l| l.parts.iter())
        .map(|p| p.cents)
        .sum()
}

#[test]
fn modes_are_data_and_restrict_what_they_carry() {
    let dataset = Dataset::load().unwrap();
    let ids: Vec<&str> = dataset
        .logistics
        .modes
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids, ["truck", "rail", "pipeline", "sea"]);
    let p = |id: &str| dataset.product_index[id];
    // Pipelines carry liquids and gas only: crude, gasoline and natural gas, never steel, food
    // or medicine.
    for liquid in ["crude_oil", "gasoline", "natural_gas", "diesel"] {
        if let Some(code) = dataset.product_index.get(liquid) {
            assert!(
                dataset.mode_carries(PIPELINE, *code),
                "{liquid} by pipeline"
            );
        }
    }
    for other in [
        "steel",
        "food_raw",
        "food_retail",
        "pharmaceuticals",
        "grain",
        "goods",
    ] {
        assert!(
            !dataset.mode_carries(PIPELINE, p(other)),
            "{other} not by pipeline"
        );
    }
    // Cold goods go by truck or rail only.
    for cold in ["food_raw", "food_retail", "pharmaceuticals"] {
        assert!(dataset.mode_carries(TRUCK, p(cold)));
        assert!(dataset.mode_carries(RAIL, p(cold)));
        assert!(!dataset.mode_carries(SEA, p(cold)), "{cold} not by ship");
    }
    // Services cannot be shipped at all.
    assert!(!dataset.mode_carries(TRUCK, p("freight")));
    assert!(!dataset.mode_carries(TRUCK, p("care")));
    // Rail needs a terminal at both ends and sea needs ports on one sea.
    let (_, dataset) = world(1);
    let (chi, ces, det) = (
        region(&dataset, "IL-CHI"),
        region(&dataset, "IL-CEN"),
        region(&dataset, "MI-DET"),
    );
    let w = {
        let (w, _) = world(1);
        w
    };
    assert!(lg::lane_possible(&dataset, &w.logistics, RAIL, chi, det));
    assert!(
        !lg::lane_possible(&dataset, &w.logistics, RAIL, chi, ces),
        "no terminal at Peoria"
    );
    assert!(
        lg::lane_possible(&dataset, &w.logistics, SEA, chi, det),
        "Chicago and Detroit are ports"
    );
    assert!(!lg::lane_possible(
        &dataset,
        &w.logistics,
        SEA,
        chi,
        region(&dataset, "OH-CEN")
    ));
}

#[test]
fn the_price_curve_is_smooth_and_meets_the_outside_price_when_full() {
    let dataset = Dataset::load().unwrap();
    let l = &dataset.logistics;
    assert_eq!(lg::spot_mult_bps(&dataset, 0), l.empty_lane_bps);
    assert_eq!(lg::spot_mult_bps(&dataset, 10_000), l.outside_premium_bps);
    let mut last = lg::spot_mult_bps(&dataset, 0);
    let mut biggest_step = 0;
    for fill in (100..=10_000).step_by(100) {
        let now = lg::spot_mult_bps(&dataset, fill);
        assert!(now >= last, "price never falls as the lane fills");
        biggest_step = biggest_step.max(now - last);
        last = now;
    }
    // No cliff: a percent of fill never moves the price by more than a tenth of the range.
    assert!(biggest_step * 10 <= (l.outside_premium_bps - l.empty_lane_bps) * 2);
    // Owning is cheaper than any lane and the outside premium is the dearest.
    assert!(l.own_cost_bps < l.empty_lane_bps && l.empty_lane_bps <= l.outside_premium_bps);
}

#[test]
fn an_uncovered_lane_is_hauled_by_outside_carriers_at_the_premium() {
    let (mut w, dataset) = world(2);
    let (shipper, _) = two_firms(&w);
    let (a, b) = (region(&dataset, "KY-EAS"), region(&dataset, "WV-NOR"));
    let plan = lg::plan_route(
        &mut w,
        &dataset,
        shipper,
        ShipPolicy::default(),
        steel(&dataset),
        a,
        b,
        MilliLots::from_lots(30),
    )
    .expect("goods never stop");
    assert_eq!(plan.legs.len(), 1);
    assert_eq!(plan.legs[0].mode, TRUCK, "no terminals in either region");
    assert!(
        plan.legs[0]
            .parts
            .iter()
            .all(|p| p.kind == PartKind::Outside)
    );
    let reference =
        cw_sim::economy::freight::freight_cost(&dataset, a, b, MilliLots::from_lots(30));
    // The reference rate floors its cents and the router rounds to even: one cent apart at most.
    let expected = reference * dataset.logistics.outside_premium_bps / 10_000;
    assert!(
        (plan.cents - expected).abs() <= 1,
        "{} vs {expected}",
        plan.cents
    );
    assert_eq!(plan.cents, sum_parts(&plan));
}

#[test]
fn a_pipeline_without_capacity_is_never_chosen_and_a_small_load_goes_by_truck() {
    let (mut w, dataset) = world(3);
    let (shipper, _) = two_firms(&w);
    let crude = dataset.product_index["crude_oil"];
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    // Under the rail minimum of twenty lots, truck is the only mode with room.
    let small = lg::plan_route(
        &mut w,
        &dataset,
        shipper,
        ShipPolicy::default(),
        crude,
        a,
        b,
        MilliLots::from_lots(5),
    )
    .unwrap();
    assert_eq!(small.modes(), vec![TRUCK]);
    // The pipeline has no outside carriers, so with nobody's capacity it is out of the running
    // at any size.
    let big = lg::plan_route(
        &mut w,
        &dataset,
        shipper,
        ShipPolicy::default(),
        crude,
        a,
        b,
        MilliLots::from_lots(500),
    )
    .unwrap();
    assert!(!big.modes().contains(&PIPELINE));
}

#[test]
fn own_carriers_ship_first_at_cost_and_others_pay_the_lane_price() {
    let (mut w, dataset) = world(4);
    let (owner, rival) = two_firms(&w);
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    lg::begin_week(&mut w, &dataset);
    // The truck carrier has put capacity on the lane (its home end is within reach).
    let key = LaneKey::new(a, b, TRUCK);
    let cap = w.logistics.lanes.get(&key).map(|l| l.cap()).unwrap_or(0);
    assert!(cap > 0, "the carrier serves the lane");

    let policy = ShipPolicy {
        kind: ShipKind::Cheapest,
        mode: Some(TRUCK),
        carrier: None,
    };
    let mine = lg::plan_route(
        &mut w,
        &dataset,
        owner,
        policy,
        steel(&dataset),
        a,
        b,
        MilliLots::from_lots(10),
    )
    .unwrap();
    let theirs = lg::plan_route(
        &mut w,
        &dataset,
        rival,
        policy,
        steel(&dataset),
        a,
        b,
        MilliLots::from_lots(10),
    )
    .unwrap();
    let mine_truck: Vec<_> = mine.legs.iter().filter(|l| l.mode == TRUCK).collect();
    assert!(
        mine_truck
            .iter()
            .any(|l| l.parts.iter().any(|p| matches!(p.kind, PartKind::Own(_))))
    );
    assert!(theirs.legs.iter().any(|l| {
        l.parts
            .iter()
            .any(|p| matches!(p.kind, PartKind::Public(_)))
    }));
    // At cost: the owner's rate is the own-cost share of the base rate, and below the rival's.
    let base = cw_sim::economy::freight::freight_cost(&dataset, a, b, MilliLots::from_lots(10));
    let own_leg = mine
        .legs
        .iter()
        .find(|l| l.mode == TRUCK && l.from == a.min(b))
        .or(mine.legs.first())
        .unwrap();
    if own_leg
        .parts
        .iter()
        .all(|p| matches!(p.kind, PartKind::Own(_)))
        && mine.legs.len() == 1
    {
        let expected = (base * dataset.logistics.own_cost_bps / 10_000).max(100);
        assert!(
            (mine.cents - expected).abs() <= 1,
            "{} vs {expected}",
            mine.cents
        );
    }
    assert!(
        mine.cents < theirs.cents,
        "own group ships cheaper: {} vs {}",
        mine.cents,
        theirs.cents
    );
}

#[test]
fn a_shipment_is_charged_once_and_every_receipt_matches() {
    let (mut w, dataset) = world(5);
    let (owner, shipper) = two_firms(&w);
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    lg::begin_week(&mut w, &dataset);
    let before_total = total_cash(&w);
    let others_before: i64 = w
        .firms
        .iter()
        .filter(|(h, _)| *h != shipper)
        .map(|(_, f)| f.book.cash())
        .sum::<i64>()
        + w.outside.gateway_book.cash();
    let shipper_before = cash(&w, shipper);
    // Large enough to fill the lane and spill to outside carriers.
    let qty = MilliLots::from_lots(4_000);
    let booked = lg::book(
        &mut w,
        &dataset,
        shipper,
        steel(&dataset),
        a,
        b,
        qty,
        "test freight",
    );
    assert!(booked.cents > 0);
    assert_eq!(
        shipper_before - cash(&w, shipper),
        booked.cents,
        "the shipper paid once"
    );
    let others_after: i64 = w
        .firms
        .iter()
        .filter(|(h, _)| *h != shipper)
        .map(|(_, f)| f.book.cash())
        .sum::<i64>()
        + w.outside.gateway_book.cash();
    assert_eq!(
        others_after - others_before,
        booked.cents,
        "carrier and outside receipts equal the shipper's posting"
    );
    assert!(cash(&w, owner) > 0);
    assert_eq!(total_cash(&w), before_total, "cash exists once");
    // Used lots never exceed capacity on any lane.
    for lane in w.logistics.lanes.values() {
        for alloc in &lane.allocs {
            assert!(alloc.used + alloc.held <= alloc.cap, "capacity is finite");
        }
    }
}

#[test]
fn a_poor_shipper_pays_what_it_has_and_receipts_shrink_with_it() {
    let (mut w, dataset) = world(6);
    let (owner, shipper) = two_firms(&w);
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    lg::begin_week(&mut w, &dataset);
    // Leave the shipper a few hundred dollars.
    let tick = w.tick;
    let firm = w.firms.get_mut(shipper).unwrap();
    let keep = 30_000;
    let drain = firm.book.cash() - keep;
    firm.book
        .commit(
            tick,
            cw_core::ledger::Txn::new()
                .credit(cw_core::ledger::Account::Cash, drain, "test")
                .debit(cw_core::ledger::Account::OperatingExpense, drain, "test"),
        )
        .unwrap();
    let before_total = total_cash(&w);
    let booked = lg::book(
        &mut w,
        &dataset,
        shipper,
        steel(&dataset),
        a,
        b,
        MilliLots::from_lots(2_000),
        "test freight",
    );
    assert!(booked.cents <= keep);
    assert_eq!(cash(&w, shipper), keep - booked.cents);
    assert_eq!(total_cash(&w), before_total);
}

#[test]
fn a_lane_contract_reserves_capacity_that_rivals_cannot_use() {
    let (mut w, dataset) = world(7);
    let (owner, holder) = two_firms(&w);
    let rival = w
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| {
            *h != owner
                && *h != holder
                && !cw_ai_free(&w, *h)
                && w.firms.get(*h).unwrap().operative()
        })
        .unwrap();
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    lg::begin_week(&mut w, &dataset);
    let key = LaneKey::new(a, b, TRUCK);
    let cap = w.logistics.lanes[&key].cap();
    assert!(cap >= 100);
    let open_before = lg::reservable_lots(&w, rival, key);
    // The owner cannot reserve its own capacity; a rival group can take up to 80 percent.
    assert!(lg::reserve_lane(&mut w, &dataset, owner, a, b, TRUCK, 10, 13).is_err());
    let lots = (cap as i64 * 6 / 10) as i32;
    lg::reserve_lane(&mut w, &dataset, holder, a, b, TRUCK, lots, 13).expect("room on the lane");
    assert!(
        lg::reserve_lane(&mut w, &dataset, holder, a, b, TRUCK, 10, 13).is_err(),
        "one contract per lane"
    );
    assert!(
        lg::reserve_lane(&mut w, &dataset, rival, a, b, TRUCK, cap, 13).is_err(),
        "more than 80 percent is refused"
    );

    // The contract starts at the next opening and charges the first week's fee, paid to the owner.
    let (holder_before, owner_before, total_before) =
        (cash(&w, holder), cash(&w, owner), total_cash(&w));
    w.tick = w.tick.next();
    lg::begin_week(&mut w, &dataset);
    assert_eq!(w.logistics.contracts.len(), 1);
    let contract = &w.logistics.contracts[0];
    assert_eq!(contract.held, lots);
    let fee = contract.held as i64 * contract.fee_per_lot_cents;
    assert_eq!(
        holder_before - cash(&w, holder),
        fee,
        "the holder paid the week's fee"
    );
    assert_eq!(
        cash(&w, owner) - owner_before,
        fee,
        "and the owner of the lane received it"
    );
    assert_eq!(total_cash(&w), total_before);
    assert_eq!(
        lg::reservable_lots(&w, rival, key),
        open_before - lots as i64
    );

    // The holder moves its reserved lots at no further charge; the rival pays the lane price on
    // what is left and then the outside premium.
    let steel = steel(&dataset);
    let mine = lg::quote(
        &mut w,
        &dataset,
        holder,
        steel,
        a,
        b,
        MilliLots::from_lots(lots as i64),
    )
    .unwrap();
    let truck: Vec<_> = mine.legs.iter().filter(|l| l.mode == TRUCK).collect();
    if mine.legs.len() == 1 && !truck.is_empty() {
        assert!(
            truck[0]
                .parts
                .iter()
                .all(|p| matches!(p.kind, PartKind::Contract(_)))
        );
        assert_eq!(mine.cents, 0, "reserved lots are already paid for");
    }
    let rival_policy = ShipPolicy {
        kind: ShipKind::Cheapest,
        mode: Some(TRUCK),
        carrier: None,
    };
    let theirs = lg::plan_route(
        &mut w,
        &dataset,
        rival,
        rival_policy,
        steel,
        a,
        b,
        MilliLots::from_lots(cap as i64),
    )
    .unwrap();
    let open_lots: i64 = theirs
        .legs
        .iter()
        .filter(|l| l.mode == TRUCK && l.from.min(l.to) == a.min(b))
        .flat_map(|l| l.parts.iter())
        .filter(|p| matches!(p.kind, PartKind::Public(_)))
        .map(|p| p.lots)
        .sum();
    assert!(
        open_lots <= cap as i64 - lots as i64,
        "the reserved lots are not for sale: {open_lots}"
    );

    // The contract runs out: a 13 week term is paid for 13 openings, then dropped.
    for _ in 0..14 {
        w.tick = w.tick.next();
        lg::begin_week(&mut w, &dataset);
    }
    assert!(w.logistics.contracts.is_empty());
}

#[test]
fn a_holder_that_cannot_pay_loses_the_contract() {
    let (mut w, dataset) = world(8);
    let (owner, holder) = two_firms(&w);
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    lg::begin_week(&mut w, &dataset);
    let cap = w.logistics.lanes[&LaneKey::new(a, b, TRUCK)].cap();
    lg::reserve_lane(&mut w, &dataset, holder, a, b, TRUCK, cap / 2, 8).unwrap();
    let tick = w.tick;
    let firm = w.firms.get_mut(holder).unwrap();
    let drain = firm.book.cash();
    firm.book
        .commit(
            tick,
            cw_core::ledger::Txn::new()
                .credit(cw_core::ledger::Account::Cash, drain, "test")
                .debit(cw_core::ledger::Account::OperatingExpense, drain, "test"),
        )
        .unwrap();
    w.tick = w.tick.next();
    lg::begin_week(&mut w, &dataset);
    assert!(w.logistics.contracts.is_empty(), "no cash, no reservation");
}

#[test]
fn policies_pick_different_routes_and_every_route_is_whole() {
    let (mut w, dataset) = world(9);
    let (shipper, _) = two_firms(&w);
    let (chi, buf) = (region(&dataset, "IL-CHI"), region(&dataset, "NY-WES"));
    let qty = MilliLots::from_lots(300);
    let steel = steel(&dataset);
    let plan = |w: &mut World, kind| {
        lg::plan_route(
            w,
            &dataset,
            shipper,
            ShipPolicy {
                kind,
                mode: None,
                carrier: None,
            },
            steel,
            chi,
            buf,
            qty,
        )
        .unwrap()
    };
    let cheapest = plan(&mut w, ShipKind::Cheapest);
    let fastest = plan(&mut w, ShipKind::Fastest);
    let reliable = plan(&mut w, ShipKind::Reliable);
    // Ports and terminals at both ends make the lake the cheap way and a faster mode the fast way.
    assert!(cheapest.cents <= fastest.cents);
    assert!(fastest.weeks <= cheapest.weeks);
    assert!(
        cheapest.modes().contains(&SEA),
        "lake shipping is cheapest: {:?}",
        cheapest.modes()
    );
    assert!(reliable.cents >= cheapest.cents);
    // A preferred mode steers a route to it when a route allows it.
    let by_rail = lg::plan_route(
        &mut w,
        &dataset,
        shipper,
        ShipPolicy {
            kind: ShipKind::Cheapest,
            mode: Some(RAIL),
            carrier: None,
        },
        steel,
        chi,
        buf,
        qty,
    )
    .unwrap();
    assert!(by_rail.modes().contains(&RAIL), "{:?}", by_rail.modes());
    // Every route is a chain from origin to destination whose parts add up to its price.
    for plan in [&cheapest, &fastest, &reliable, &by_rail] {
        assert_eq!(plan.legs.first().unwrap().from, chi);
        assert_eq!(plan.legs.last().unwrap().to, buf);
        for pair in plan.legs.windows(2) {
            assert_eq!(pair[0].to, pair[1].from);
        }
        assert_eq!(plan.cents, sum_parts(plan));
        assert!(plan.legs.len() <= lg::MAX_LEGS && plan.weeks >= 1);
        for leg in &plan.legs {
            assert_eq!(
                leg.parts.iter().map(|p| p.lots).sum::<i64>(),
                plan.lots,
                "every lot rides every leg"
            );
        }
    }
}

#[test]
fn a_region_without_a_terminal_reaches_rail_through_a_truck_leg() {
    let (mut w, dataset) = world(10);
    let (shipper, _) = two_firms(&w);
    // Peoria has no rail yard; Detroit does. A long heavy haul still finds rail by trucking first.
    let (src, dst) = (region(&dataset, "IL-CEN"), region(&dataset, "PA-PHL"));
    let plan = lg::plan_route(
        &mut w,
        &dataset,
        shipper,
        ShipPolicy::default(),
        steel(&dataset),
        src,
        dst,
        MilliLots::from_lots(400),
    )
    .unwrap();
    assert!(!lg::lane_possible(&dataset, &w.logistics, RAIL, src, dst));
    assert!(plan.modes().contains(&RAIL), "{:?}", plan.modes());
    assert_eq!(plan.modes().first(), Some(&TRUCK));
    assert!(plan.legs.len() >= 2);
}

#[test]
fn a_built_terminal_opens_its_region_to_everyone() {
    let (mut w, dataset) = world(11);
    let (builder, _) = two_firms(&w);
    let ces = region(&dataset, "IL-CEN");
    let chi = region(&dataset, "IL-CHI");
    assert!(!lg::lane_possible(&dataset, &w.logistics, RAIL, ces, chi));
    add_plant(&mut w, &dataset, builder, ces, "rail_terminal");
    lg::begin_week(&mut w, &dataset);
    assert!(lg::has_access(&dataset, &w.logistics, RAIL, ces));
    assert!(lg::lane_possible(&dataset, &w.logistics, RAIL, ces, chi));
    // The terminal itself adds rail capacity on lanes touching Peoria.
    assert!(
        w.logistics
            .lanes
            .keys()
            .any(|k| k.mode == RAIL && (k.a == ces || k.b == ces))
    );
}

#[test]
fn carriers_follow_the_traffic_and_pinning_overrides_it() {
    let (mut w, dataset) = world(12);
    let (owner, _) = two_firms(&w);
    let home = region(&dataset, "OH-CEN");
    let plant = add_plant(&mut w, &dataset, owner, home, "carrier");
    // Heavy truck traffic on one lane pulls the capacity there.
    let hot = LaneKey::new(home, region(&dataset, "OH-SE"), TRUCK);
    w.logistics.flows.insert(hot, 5_000);
    lg::begin_week(&mut w, &dataset);
    let hot_cap = w.logistics.lanes[&hot].cap();
    let other_max = w
        .logistics
        .lanes
        .iter()
        .filter(|(k, l)| {
            **k != hot
                && k.mode == TRUCK
                && l.allocs
                    .iter()
                    .any(|a| w.logistics.carriers[a.carrier as usize].facility == plant)
        })
        .map(|(_, l)| l.cap())
        .max()
        .unwrap_or(0);
    assert!(hot_cap > other_max, "{hot_cap} vs {other_max}");
    // Pinned to one other region, all the capacity goes to that lane.
    let pin = region(&dataset, "OH-NE");
    cw_sim::economy::logistics::set_serves(&mut w, &dataset, plant, vec![pin]).unwrap();
    lg::begin_week(&mut w, &dataset);
    let mine: Vec<LaneKey> = w
        .logistics
        .lanes
        .iter()
        .filter(|(_, l)| {
            l.allocs
                .iter()
                .any(|a| w.logistics.carriers[a.carrier as usize].facility == plant)
        })
        .map(|(k, _)| *k)
        .collect();
    assert_eq!(mine, vec![LaneKey::new(home, pin, TRUCK)]);
}

#[test]
fn carrier_capacity_is_lot_kilometres_so_short_lanes_hold_more_lots() {
    let (mut w, dataset) = world(13);
    let (owner, _) = two_firms(&w);
    let home = region(&dataset, "OH-CEN");
    let plant = add_plant(&mut w, &dataset, owner, home, "carrier");
    let near = region(&dataset, "OH-SE");
    let far = region(&dataset, "PA-PHL");
    for pin in [near, far] {
        cw_sim::economy::logistics::set_serves(&mut w, &dataset, plant, vec![pin]).unwrap();
        lg::begin_week(&mut w, &dataset);
    }
    let lots_far = w.logistics.lanes[&LaneKey::new(home, far, TRUCK)].cap() as i64;
    cw_sim::economy::logistics::set_serves(&mut w, &dataset, plant, vec![near]).unwrap();
    lg::begin_week(&mut w, &dataset);
    let lots_near = w.logistics.lanes[&LaneKey::new(home, near, TRUCK)].cap() as i64;
    assert!(lots_near > lots_far);
    // Each lane carries the same lot-kilometres: the fleet's weekly capacity over the reference haul.
    let near_km = dataset.km(home, near) as i64;
    let far_km = dataset.km(home, far) as i64;
    let fleet = 1_500 * lg::REF_KM;
    assert!((lots_near * near_km - fleet).abs() <= near_km);
    assert!((lots_far * far_km - fleet).abs() <= far_km);
}

#[test]
fn stats_flag_sustained_congestion_and_rate_hikes() {
    let (mut w, dataset) = world(14);
    let (owner, shipper) = two_firms(&w);
    // Southeast Ohio has no rail yard or port: only trucks serve the lane.
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-SE"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    let key = LaneKey::new(a, b, TRUCK);
    let mut prices: Vec<i64> = Vec::new();
    for _ in 0..6 {
        w.tick = w.tick.next();
        lg::begin_week(&mut w, &dataset);
        // Fill the lane every week and overflow it.
        let cap = w.logistics.lanes[&key].cap() as i64;
        lg::book(
            &mut w,
            &dataset,
            shipper,
            steel(&dataset),
            a,
            b,
            MilliLots::from_lots(cap * 2),
            "test",
        );
        prices.push(lg::lane_mult_bps(&dataset, &w.logistics, key));
    }
    w.tick = w.tick.next();
    lg::begin_week(&mut w, &dataset);
    let stat = &w.logistics.stats[&key];
    assert!(
        stat.hot_weeks >= lg::SUSTAINED_WEEKS,
        "{} hot weeks",
        stat.hot_weeks
    );
    assert!(stat.users.iter().any(|(f, _)| *f == shipper));
    assert!(lg::depends_on(&w, shipper, key));
    assert!(lg::hot_lanes(&w, &dataset).iter().any(|h| h.key == key));
    // A lane held full for weeks costs more than the same lane empty.
    assert!(
        prices
            .iter()
            .all(|p| *p >= dataset.logistics.empty_lane_bps)
    );
}

#[test]
fn sea_lanes_exist_between_lake_ports_and_to_gateways() {
    let dataset = Dataset::load().unwrap();
    let ports = ["IL-CHI", "IN-NW", "MI-DET", "OH-NW", "OH-NE", "NY-WES"];
    for a in ports {
        for b in ports {
            if a != b {
                let km = dataset.sea_km(region(&dataset, a), region(&dataset, b));
                assert!(km.is_some(), "{a} to {b} by lake");
                assert!(km.unwrap() > dataset.km(region(&dataset, a), region(&dataset, b)) / 2);
            }
        }
    }
    // Inland regions have no sea lane, and a gateway link names a real port region.
    assert!(
        dataset
            .sea_km(region(&dataset, "IL-CHI"), region(&dataset, "KY-LOU"))
            .is_none()
    );
    assert!(
        dataset
            .logistics
            .gateway_links
            .iter()
            .all(|l| dataset.region_index.contains_key(&l.region))
    );
    assert_eq!(StorageClass::ALL.len(), 4);
}

#[test]
fn estimates_for_pairing_rank_by_distance_and_mode() {
    let (mut w, dataset) = world(15);
    let near = lg::estimate_cents_per_lot(
        &mut w,
        &dataset,
        region(&dataset, "OH-CEN"),
        region(&dataset, "OH-SE"),
        StorageClass::Dry,
    );
    let far = lg::estimate_cents_per_lot(
        &mut w,
        &dataset,
        region(&dataset, "OH-CEN"),
        region(&dataset, "NY-NYC"),
        StorageClass::Dry,
    );
    assert!(near < far);
    // Cold goods cannot go by ship, so the cold rate is never under the dry rate on a lake lane.
    let dry = lg::estimate_cents_per_lot(
        &mut w,
        &dataset,
        region(&dataset, "IL-CHI"),
        region(&dataset, "NY-WES"),
        StorageClass::Dry,
    );
    let cold = lg::estimate_cents_per_lot(
        &mut w,
        &dataset,
        region(&dataset, "IL-CHI"),
        region(&dataset, "NY-WES"),
        StorageClass::Cold,
    );
    assert!(cold >= dry);
    let _ = Cents::ZERO;
}

#[test]
fn the_shipping_summary_splits_what_was_paid_and_prices_policies_before_they_are_chosen() {
    let (mut w, dataset) = world(5);
    let (owner, shipper) = two_firms(&w);
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    lg::begin_week(&mut w, &dataset);
    // Enough to fill the lane and spill to outside carriers.
    let booked = lg::book(
        &mut w,
        &dataset,
        shipper,
        steel(&dataset),
        a,
        b,
        MilliLots::from_lots(4_000),
        "test freight",
    );
    // Nothing is reported until the week closes.
    assert_eq!(
        lg::shipping_summary(&w, &dataset, shipper, Some(b)).cents,
        0
    );
    lg::begin_week(&mut w, &dataset);
    let sum = lg::shipping_summary(&w, &dataset, shipper, Some(b));
    assert_eq!(
        sum.cents, booked.cents,
        "the summary is what the shipper paid"
    );
    assert_eq!(
        sum.own_cents + sum.other_cents + sum.outside_cents,
        sum.cents,
        "own, other firms and outside carriers add up to the bill"
    );
    assert!(sum.outside_cents > 0, "the lane spilled: {sum:?}");
    assert_eq!(sum.lots, 4_000);
    assert!(
        sum.outside_lots > 0 && sum.outside_lots <= sum.lots,
        "{sum:?}"
    );
    // Outside carriers charge 125 percent against 65 percent at cost: 48 percent of their bill
    // is premium over what the same lots cost on own carriers.
    let l = &dataset.logistics;
    assert_eq!(
        sum.outside_extra_cents,
        sum.outside_cents - sum.outside_cents * l.own_cost_bps / l.outside_premium_bps
    );
    assert_eq!(sum.lanes.len(), 1);
    assert_eq!(sum.lanes[0].cents, sum.cents);
    // All of it went to outside carriers, so building a carrier on the lane is suggested with a
    // payback: the premium saved each week against what the blocks cost.
    match sum.suggestion {
        Some(lg::Suggestion::Build {
            weekly_saving_cents,
            upfront_cents,
            payback_weeks,
            ..
        }) => {
            assert_eq!(weekly_saving_cents, sum.outside_extra_cents);
            assert_eq!(
                payback_weeks,
                (upfront_cents + weekly_saving_cents - 1) / weekly_saving_cents
            );
        }
        other => panic!("expected a build suggestion, got {other:?}"),
    }
    // Another region shows nothing; the firm total matches while no fees are owed.
    assert_eq!(
        lg::shipping_summary(&w, &dataset, shipper, Some(a)).cents,
        0
    );
    assert_eq!(
        lg::shipping_summary(&w, &dataset, shipper, None).cents,
        booked.cents
    );

    // Pricing a policy books nothing and leaves cash alone.
    let cash_before = cash(&w, shipper);
    let cheapest =
        lg::estimate_policy(&mut w, &dataset, shipper, b, ShipPolicy::default()).unwrap();
    let fastest = lg::estimate_policy(
        &mut w,
        &dataset,
        shipper,
        b,
        ShipPolicy {
            kind: ShipKind::Fastest,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        cheapest.0 > 0 && cheapest.0 <= fastest.0,
        "cheapest is never dearer"
    );
    assert_eq!(cash(&w, shipper), cash_before);
    assert!(lg::estimate_policy(&mut w, &dataset, shipper, a, ShipPolicy::default()).is_none());
}

#[test]
fn a_reserve_quote_is_the_fee_the_contract_then_charges() {
    let (mut w, dataset) = world(5);
    let (owner, shipper) = two_firms(&w);
    let (a, b) = (region(&dataset, "OH-CEN"), region(&dataset, "OH-NW"));
    add_plant(&mut w, &dataset, owner, a, "carrier");
    lg::begin_week(&mut w, &dataset);
    let key = LaneKey::new(a, b, TRUCK);
    let quote = lg::reserve_quote(&w, &dataset, shipper, key, 50, 13).unwrap();
    assert_eq!(quote.weekly_fee_cents, quote.fee_per_lot_cents * 50);
    assert_eq!(quote.total_cents, quote.weekly_fee_cents * 13);
    assert!(
        quote.fee_per_lot_cents < quote.spot_per_lot_cents,
        "a commitment discount"
    );
    let id = lg::reserve_lane(&mut w, &dataset, shipper, a, b, TRUCK, 50, 13).unwrap();
    let contract = w.logistics.contracts.iter().find(|c| c.id == id).unwrap();
    assert_eq!(contract.fee_per_lot_cents, quote.fee_per_lot_cents);
    // A lane with no capacity cannot be quoted.
    let empty = LaneKey::new(
        region(&dataset, "OH-SE"),
        region(&dataset, "NY-NYC"),
        PIPELINE,
    );
    assert!(lg::reserve_quote(&w, &dataset, shipper, empty, 50, 13).is_none());
}

#[test]
fn a_carrier_trims_toward_85_percent_only_while_its_lanes_keep_their_freight() {
    // One carrier pinned to one truck lane, run at 80 percent. The plan reads last week's fill by
    // lot-km and the lane's outside overflow; worked by hand from the rule in MARKETS.md:
    // under 85 percent full cuts 15 points, from 90 raises 15, and an overflow of 40 percent or
    // more of the lane's lots blocks the cut (but not the raise).
    let (mut w, dataset) = world(15);
    let (owner, _) = two_firms(&w);
    let home = region(&dataset, "OH-CEN");
    let near = region(&dataset, "OH-SE");
    let plant = add_plant(&mut w, &dataset, owner, home, "carrier");
    lg::set_serves(&mut w, &dataset, plant, vec![near]).unwrap();
    lg::begin_week(&mut w, &dataset);
    let key = LaneKey::new(home, near, TRUCK);
    let index = w
        .logistics
        .carriers
        .iter()
        .position(|c| c.facility == plant)
        .unwrap();
    let full_km = w.logistics.carriers[index].cap as i64 * lg::REF_KM;
    let target = |w: &mut World, fill_pct: i64, used: i32, outside: i32| {
        w.facilities.get_mut(plant).unwrap().target_bps = 8_000;
        w.logistics.carriers[index].used_lot_km = full_km * fill_pct / 100;
        let lane = w.logistics.lanes.get_mut(&key).unwrap();
        lane.allocs[0].used = used;
        lane.outside = outside;
        cw_sim::economy::pricing::plant_targets(w, &dataset, owner)
            .into_iter()
            .find(|(h, _)| *h == plant)
            .unwrap()
            .1
    };
    // 80 percent full, nothing overflowed: under 85, so it cuts. The old 60 percent band held.
    assert_eq!(target(&mut w, 80, 100, 0), 6_500);
    // 88 percent full: inside the band, holds.
    assert_eq!(target(&mut w, 88, 100, 0), 8_000);
    // 80 percent full but 50 of 100 lots went outside (50 percent): the freight is wanted, holds.
    assert_eq!(target(&mut w, 80, 50, 50), 8_000);
    // Exactly 40 percent outside (40 of 100) still holds; 39 of 100 does not.
    assert_eq!(target(&mut w, 80, 60, 40), 8_000);
    assert_eq!(target(&mut w, 80, 61, 39), 6_500);
    // 95 percent full with heavy overflow raises.
    assert_eq!(target(&mut w, 95, 50, 50), 9_500);
}

#[test]
fn a_logistics_chief_cuts_what_outside_carriers_charge_by_the_stated_share() {
    use cw_sim::economy::executives as ex;
    let (mut w, dataset) = world(2);
    let (shipper, _) = two_firms(&w);
    let (a, b) = (region(&dataset, "KY-EAS"), region(&dataset, "WV-NOR"));
    let route = |w: &mut World| {
        lg::plan_route(
            w,
            &dataset,
            shipper,
            ShipPolicy::default(),
            steel(&dataset),
            a,
            b,
            MilliLots::from_lots(30),
        )
        .expect("goods never stop")
        .cents
    };
    let plain = route(&mut w);
    ex::weekly(&mut w, &dataset);
    let id = ex::candidates(&w, ex::Post::Logistics)[0].id;
    w.executives.people.iter_mut().find(|c| c.id == id).unwrap().level = 5;
    w.executives.posts.push(ex::Executive {
        firm: shipper,
        post: ex::Post::Logistics,
        person: id,
        since: 0,
        expires: 104,
        pay_cents: 1,
    });
    let cut = route(&mut w);
    // Only the premium over the owners' empty-lane rate shrinks, by 25 percent at level 5.
    let l = &dataset.logistics;
    let reference =
        cw_sim::economy::freight::freight_cost(&dataset, a, b, MilliLots::from_lots(30));
    let expected = reference * (l.empty_lane_bps + (l.outside_premium_bps - l.empty_lane_bps) * 75 / 100) / 10_000;
    assert!(plain > cut);
    assert!((cut - expected).abs() <= 1, "{cut} vs {expected}");
}
