//! Rival long plans (1.9) and workforce migration: stake accumulation respects the float, price
//! impact and the disclosure line, the abandon rules sell the stake back, and migration conserves
//! people, savings and demand while responding to the pay gap.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::economy::plans::{self, Action, EventKind, Outcome, Plan, Stage};
use cw_sim::economy::{equity, migration, seed};
use cw_sim::state::{HolderId, ScenarioKind, ScenarioMode, World};

fn world() -> (World, Dataset) {
    let ds = Dataset::load().unwrap();
    let w = seed::build(&ds, 1, ScenarioKind::Guided, ScenarioMode::Sandbox);
    (w, ds)
}

fn by_name(w: &World, name: &str) -> Handle {
    w.firms
        .iter()
        .find(|(_, f)| f.name == name)
        .map(|(h, _)| h)
        .unwrap()
}

fn fund(w: &mut World, firm: Handle, cents: i64) {
    w.firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            w.tick,
            Txn::new().debit(Account::Cash, cents, "fixture").credit(
                Account::ShareCapital,
                cents,
                "fixture",
            ),
        )
        .unwrap();
}

fn send(w: &mut World, ds: &Dataset, actor: Handle, intent: CommandIntent) {
    let env = CommandEnvelope {
        command_id_high: 9,
        command_id_low: w.revision,
        actor: actor.0,
        expected_revision: w.revision,
        submitted_tick: w.tick.0,
        intent,
    };
    cw_sim::commands::apply(w, ds, &env).expect("command accepted");
}

/// A raider with a temperament and a listed target with a wide float, and a plan between them.
fn plan_world() -> (World, Dataset, Handle, Handle) {
    let (mut w, ds) = world();
    let raider = by_name(&w, "Great Lakes Fiber");
    let target = by_name(&w, "Cedar Bend Software");
    fund(&mut w, raider, 100_000_000_000);
    w.tick = Tick(30);
    let f = w.firms.get(target).unwrap();
    let plan = Plan {
        raider,
        target,
        temperament: cw_data::Temperament::Financier.code(),
        stage: Stage::Building,
        started: 30,
        stage_since: 30,
        entry_mark_cents: f.shares.mark_cents,
        entry_fair_cents: f.shares.fair_cents,
        base_held: 0,
        disclosed: None,
        deal: None,
        stake_bps: 0,
        reason: None,
        history: Vec::new(),
    };
    w.plans.plans.push(plan);
    (w, ds, raider, target)
}

fn held(w: &World, raider: Handle, target: Handle) -> i64 {
    w.firms
        .get(target)
        .unwrap()
        .shares
        .qty(HolderId::Firm(raider))
}

fn all_holders(w: &World, target: Handle) -> i64 {
    w.firms
        .get(target)
        .unwrap()
        .shares
        .holders
        .values()
        .map(|l| l.qty)
        .sum()
}

#[test]
fn a_step_buys_at_most_half_the_float_and_pays_the_impact_price() {
    let (mut w, ds, raider, target) = plan_world();
    let before = held(&w, raider, target);
    let holders_before = all_holders(&w, target);
    let mark = w.firms.get(target).unwrap().shares.mark_cents;
    let outside = plans::outside_held(&w, target);
    let float = plans::weekly_float(&w, target);
    assert_eq!(float, outside * 2 / 100);
    let Some(Action::Buy {
        qty, limit_cents, ..
    }) = plans::rival_action(&w, &ds, raider)
    else {
        panic!("a plan in its building stage buys");
    };
    assert_eq!(qty, float / 2, "half the executable float and no more");
    assert_eq!(
        limit_cents,
        mark + mark * 2 / 100,
        "no more than two percent over the mark"
    );
    let cash = w.firms.get(raider).unwrap().book.cash();
    send(
        &mut w,
        &ds,
        raider,
        CommandIntent::PlaceShareOrder {
            investor: false,
            firm: target,
            buy: true,
            qty,
            limit_price_cents: limit_cents,
        },
    );
    equity::clear_exchange(&mut w, &ds);
    let bought = held(&w, raider, target) - before;
    assert_eq!(bought, qty, "the order fills in full inside the float");
    assert!(bought <= float);
    // Oracle: impact is ten basis points per percent of the outside holdings the step takes,
    // and the buyer pays mark plus that impact on every share.
    let impact_bps = qty * 10_000 / outside / 10;
    let price = mark + mark * impact_bps / 10_000;
    let lot = w.firms.get(target).unwrap().shares.holders[&HolderId::Firm(raider)];
    assert_eq!(lot.cost_cents, qty * price);
    // The unused part of the reserved limit comes straight back, and the cash that left is
    // exactly the price paid.
    let book = &w.firms.get(raider).unwrap().book;
    assert_eq!(book.cash(), cash - qty * price);
    assert_eq!(book.balance(Account::RestrictedCash), 0);
    // No share was made up: the holders still add up to what they did before.
    assert_eq!(all_holders(&w, target), holders_before);
}

#[test]
fn an_order_the_float_cannot_fill_is_released_in_full() {
    let (mut w, ds, raider, target) = plan_world();
    let cash = w.firms.get(raider).unwrap().book.cash();
    let float = plans::weekly_float(&w, target);
    // Twice the float cannot fill past the float; the rest of the reservation comes back.
    let mark = w.firms.get(target).unwrap().shares.mark_cents;
    send(
        &mut w,
        &ds,
        raider,
        CommandIntent::PlaceShareOrder {
            investor: false,
            firm: target,
            buy: true,
            qty: float * 2,
            limit_price_cents: mark * 2,
        },
    );
    equity::clear_exchange(&mut w, &ds);
    let got = held(&w, raider, target);
    assert_eq!(got, float, "the first week fills the float and no more");
    // The order stays for one more clearing, then expires and its reserve comes back.
    w.tick = Tick(w.tick.0 + 1);
    equity::clear_exchange(&mut w, &ds);
    let got = held(&w, raider, target);
    assert!(
        got <= 2 * float,
        "never past the float in a week: {got} vs {float}"
    );
    let lot = w.firms.get(target).unwrap().shares.holders[&HolderId::Firm(raider)];
    let book = &w.firms.get(raider).unwrap().book;
    assert_eq!(book.cash(), cash - lot.cost_cents);
    assert_eq!(book.balance(Account::RestrictedCash), 0);
}

#[test]
fn crossing_five_percent_is_disclosed_and_four_point_nine_is_not() {
    let (mut w, ds, raider, target) = plan_world();
    let eligible = w.firms.get(target).unwrap().shares.eligible_votes();
    let give = |w: &mut World, bps: i64| {
        let from = HolderId::Outside(1);
        let have = held(w, raider, target);
        let want = eligible * bps / 10_000;
        let f = w.firms.get_mut(target).unwrap();
        if want > have {
            f.shares.transfer(
                from,
                HolderId::Firm(raider),
                want - have,
                (want - have) * 500,
            );
        }
    };
    give(&mut w, 490);
    plans::weekly(&mut w, &ds);
    assert!(w.plans.plans[0].disclosed.is_none());
    assert!(
        w.plans.plans[0].history.is_empty(),
        "nothing is public below the line"
    );
    assert!(plans::disclosed_stakes(&w, target).is_empty());
    assert!(
        plans::likely_next_move(&w, raider).is_none(),
        "intent below the line stays hidden"
    );
    give(&mut w, 510);
    w.tick = Tick(31);
    plans::weekly(&mut w, &ds);
    assert_eq!(w.plans.plans[0].disclosed, Some(31));
    assert_eq!(w.plans.events.last().unwrap().kind, EventKind::Disclosed);
    let shown = plans::disclosed_stakes(&w, target);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].holder, raider);
    assert_eq!(shown[0].bps, 510);
    assert!(
        plans::likely_next_move(&w, raider)
            .unwrap()
            .contains("5 percent")
    );
}

#[test]
fn a_quarter_of_the_votes_launches_the_tender_at_the_standard_premium() {
    let (mut w, ds, raider, target) = plan_world();
    let eligible = w.firms.get(target).unwrap().shares.eligible_votes();
    w.firms.get_mut(target).unwrap().shares.transfer(
        HolderId::Outside(1),
        HolderId::Firm(raider),
        eligible / 4,
        eligible / 4 * 500,
    );
    plans::weekly(&mut w, &ds);
    assert_eq!(w.plans.plans[0].stage, Stage::Tender);
    let mark = w.firms.get(target).unwrap().shares.mark_cents;
    let held_now = held(&w, raider, target);
    let Some(Action::Tender {
        price_cents,
        shares,
        ..
    }) = plans::rival_action(&w, &ds, raider)
    else {
        panic!("tender stage launches the bid");
    };
    assert_eq!(price_cents, mark + mark * 25 / 100);
    assert_eq!(shares, eligible / 2 + 1 - held_now);
    // Through the ordinary takeover command, and the plan then follows that deal.
    send(
        &mut w,
        &ds,
        raider,
        CommandIntent::StartDiligence {
            buyer: raider,
            target,
            kind: cw_sim::state::DealKind::SharePurchase,
            price_cents,
            max_shares: shares,
        },
    );
    w.tick = Tick(w.tick.0 + 1);
    plans::weekly(&mut w, &ds);
    let deal = w.plans.plans[0].deal.expect("the plan found its deal");
    assert!(w.deals.get(deal).is_some());
    assert_eq!(w.plans.stats.tenders, 1);
    assert!(
        plans::likely_next_move(&w, raider)
            .unwrap()
            .contains("offer")
    );
}

#[test]
fn a_price_that_runs_away_abandons_the_plan_and_the_stake_is_sold_back() {
    let (mut w, ds, raider, target) = plan_world();
    let eligible = w.firms.get(target).unwrap().shares.eligible_votes();
    let holders_before = all_holders(&w, target);
    let cost = eligible * 8 / 100 * 500;
    w.firms.get_mut(target).unwrap().shares.transfer(
        HolderId::Outside(1),
        HolderId::Firm(raider),
        eligible * 8 / 100,
        cost,
    );
    plans::weekly(&mut w, &ds);
    assert_eq!(w.plans.plans[0].stage, Stage::Building);
    // A financier's cap is thirty percent over the entry mark. Just under it, the plan holds.
    let entry = w.plans.plans[0].entry_mark_cents;
    w.firms.get_mut(target).unwrap().shares.mark_cents = entry * 129 / 100;
    w.tick = Tick(31);
    plans::weekly(&mut w, &ds);
    assert_eq!(w.plans.plans[0].stage, Stage::Building);
    w.firms.get_mut(target).unwrap().shares.mark_cents = entry * 131 / 100;
    w.tick = Tick(32);
    plans::weekly(&mut w, &ds);
    assert_eq!(w.plans.plans[0].stage, Stage::SellingOut);
    assert_eq!(w.plans.plans[0].reason, Some(Outcome::PriceRan));
    assert_eq!(w.plans.events.last().unwrap().kind, EventKind::Abandoned);
    // It sells step by step through the same order, never more than half the float.
    let mut weeks = 0;
    while !w.plans.plans.is_empty() && weeks < 60 {
        w.tick = Tick(w.tick.0 + 1);
        let float = plans::weekly_float(&w, target);
        if let Some(Action::Sell {
            qty,
            target: t,
            limit_cents,
        }) = plans::rival_action(&w, &ds, raider)
        {
            assert!(qty <= (float / 2).max(1));
            send(
                &mut w,
                &ds,
                raider,
                CommandIntent::PlaceShareOrder {
                    investor: false,
                    firm: t,
                    buy: false,
                    qty,
                    limit_price_cents: limit_cents,
                },
            );
        }
        equity::clear_exchange(&mut w, &ds);
        plans::weekly(&mut w, &ds);
        weeks += 1;
    }
    assert!(
        w.plans.plans.is_empty(),
        "the plan ended once the stake was gone"
    );
    assert_eq!(held(&w, raider, target), 0);
    let ended = w.plans.ended.last().unwrap();
    assert_eq!(ended.outcome, Outcome::PriceRan);
    assert!(
        ended.peak_bps >= 790,
        "history keeps the peak: {}",
        ended.peak_bps
    );
    assert!(
        w.plans.cooldown[&raider] > w.tick.0,
        "no new plan for two years"
    );
    // The books stay whole: nothing was left in restricted cash and the cost of the shares left
    // the investments account when they were sold.
    let book = &w.firms.get(raider).unwrap().book;
    assert_eq!(book.balance(Account::RestrictedCash), 0);
    assert_eq!(all_holders(&w, target), holders_before);
}

#[test]
fn another_holder_taking_control_ends_the_plan_a_white_knight() {
    let (mut w, ds, _, target) = plan_world();
    let eligible = w.firms.get(target).unwrap().shares.eligible_votes();
    let knight = by_name(&w, "Forgeworks Ordnance");
    w.firms.get_mut(target).unwrap().shares.transfer(
        HolderId::Outside(1),
        HolderId::Firm(knight),
        eligible / 2 + 1,
        1,
    );
    plans::weekly(&mut w, &ds);
    assert_eq!(w.plans.plans[0].stage, Stage::SellingOut);
    assert_eq!(w.plans.plans[0].reason, Some(Outcome::Controlled));
}

#[test]
fn a_campaign_past_its_time_limit_gives_up() {
    let (mut w, ds, _, _) = plan_world();
    w.tick = Tick(30 + plans::MAX_WEEKS + 1);
    plans::weekly(&mut w, &ds);
    assert_eq!(w.plans.plans[0].reason, Some(Outcome::TimedOut));
}

#[test]
fn the_target_buys_back_shares_against_a_disclosed_stake_but_never_into_the_raiders_control() {
    let (mut w, ds, raider, target) = plan_world();
    let eligible = w.firms.get(target).unwrap().shares.eligible_votes();
    let give = |w: &mut World, bps: i64| {
        w.firms.get_mut(target).unwrap().shares.transfer(
            HolderId::Outside(1),
            HolderId::Firm(raider),
            eligible * bps / 10_000,
            1,
        );
    };
    give(&mut w, 1_200);
    plans::weekly(&mut w, &ds);
    // The firm answers on its own cadence (every fourth week).
    let mut asked = None;
    for t in 31..39 {
        w.tick = Tick(t);
        if let Some(q) = plans::rival_defence(&w, target) {
            asked = Some(q);
            break;
        }
    }
    let qty = asked.expect("a rival target with cash buys back");
    assert_eq!(qty, eligible / 100);
    send(
        &mut w,
        &ds,
        target,
        CommandIntent::RepurchaseShares { firm: target, qty },
    );
    assert_eq!(w.firms.get(target).unwrap().shares.treasury(), qty);
    // Close to the control line, the answer stops: a buyback would hand over control.
    let (mut w2, _ds2, raider2, target2) = plan_world();
    let eligible2 = w2.firms.get(target2).unwrap().shares.eligible_votes();
    w2.firms.get_mut(target2).unwrap().shares.transfer(
        HolderId::Outside(1),
        HolderId::Firm(raider2),
        eligible2 * 3_990 / 10_000,
        1,
    );
    w2.tick = Tick(31);
    plans::weekly(&mut w2, &ds);
    for t in 32..40 {
        w2.tick = Tick(t);
        assert!(plans::rival_defence(&w2, target2).is_none());
    }
}

#[test]
fn only_temperaments_that_take_over_open_plans_and_only_against_a_discount() {
    let (mut w, ds) = world();
    w.tick = Tick(plans::FIRST_WEEK);
    // Nothing is cheap at the opening, so a raider (which needs a discount) finds no target.
    for tick in plans::FIRST_WEEK..plans::FIRST_WEEK + 13 {
        w.tick = Tick(tick);
        plans::weekly(&mut w, &ds);
    }
    assert!(
        w.plans.plans.iter().all(|p| p.temperament != 1),
        "no raider plan against fairly priced firms"
    );
    // Mark a listed firm well under its fair estimate: a raider with the cash now opens a plan.
    let target = by_name(&w, "Cedar Bend Software");
    {
        let f = w.firms.get_mut(target).unwrap();
        f.shares.mark_cents = f.shares.fair_cents * 70 / 100;
    }
    let raider = by_name(&w, "Forgeworks Ordnance");
    fund(&mut w, raider, 200_000_000_000);
    for tick in plans::FIRST_WEEK + 13..plans::FIRST_WEEK + 40 {
        w.tick = Tick(tick);
        plans::weekly(&mut w, &ds);
        if w.plans.plan_of(raider).is_some() {
            break;
        }
    }
    let plan = w
        .plans
        .plan_of(raider)
        .expect("the raider opens a plan against the discount");
    assert_eq!(plan.target, target);
    assert_eq!(plan.stage, Stage::Building);
    // One plan per target, and none for a firm that has no ear for takeovers.
    assert!(w.plans.plans.iter().filter(|p| p.target == target).count() == 1);
    let builder = by_name(&w, "Crossroads Carriers");
    assert!(w.plans.plan_of(builder).is_none());
}

// ---- Migration ------------------------------------------------------------------------------

fn totals(w: &World) -> (i64, i64, i64, i64) {
    let mut people = 0;
    let mut savings = 0;
    let mut income = 0;
    let mut lots = 0;
    for r in &w.regions {
        people += r.labour.iter().map(|l| l.population).sum::<i64>();
        savings += r.households.savings_cents;
        income += r.households.baseline_income_cents;
        lots += r.households.baseline_lots.values().sum::<i64>();
    }
    (people, savings, income, lots)
}

fn run_weeks(w: &mut World, ds: &Dataset, weeks: u32) {
    for _ in 0..weeks {
        w.tick = Tick(w.tick.0 + 1);
        migration::weekly(w, ds);
        cw_sim::economy::labour::clear(w, ds);
    }
}

#[test]
fn no_pay_gap_means_no_migration() {
    let (mut w, ds) = world();
    run_weeks(&mut w, &ds, 40);
    assert!(
        w.plans.migration.total.iter().all(|t| *t == 0),
        "{:?}",
        w.plans.migration.total
    );
}

#[test]
fn workers_move_toward_the_better_paying_region_and_nothing_is_lost() {
    let (mut w, ds) = world();
    run_weeks(&mut w, &ds, 2);
    let before = totals(&w);
    // Region 1 starts paying a quarter more in every tier, a gap well over the threshold.
    let (a, b) = (0usize, 1usize);
    for tier in 0..3 {
        let l = &mut w.regions[b].labour[tier];
        l.prevailing_wage_cents = l.prevailing_wage_cents * 125 / 100;
    }
    let pop_b = w.regions[b]
        .labour
        .iter()
        .map(|l| l.population)
        .sum::<i64>();
    run_weeks(&mut w, &ds, 104);
    let after = totals(&w);
    assert_eq!(
        before, after,
        "people, savings, baseline income and baseline demand are conserved"
    );
    let net_b = w.plans.migration.total[b];
    assert!(net_b > 0, "the better paying region gains workers: {net_b}");
    let pop_b_after = w.regions[b]
        .labour
        .iter()
        .map(|l| l.population)
        .sum::<i64>();
    assert_eq!(pop_b_after - pop_b, net_b);
    assert!(w.plans.migration.total[a] <= 0 || w.plans.migration.total.iter().any(|t| *t < 0));
    // A few percent a year at most: two years is below ten percent of the destination.
    assert!(net_b * 100 / pop_b < 10, "{net_b} of {pop_b}");
    // The dossier figure is the sum of the last 52 weekly entries.
    let year: i64 = w.plans.migration.net[b]
        .iter()
        .rev()
        .take(52)
        .map(|n| *n as i64)
        .sum();
    assert_eq!(w.plans.migration.net_over(b, 52), year);
}

#[test]
fn a_bigger_gap_pulls_more_workers_and_distance_and_borders_hold_them_back() {
    let (mut w0, ds) = world();
    run_weeks(&mut w0, &ds, 2);
    let flow = |boost: i64| {
        let mut w = w0.clone();
        for tier in 0..3 {
            let l = &mut w.regions[1].labour[tier];
            l.prevailing_wage_cents = l.prevailing_wage_cents * (100 + boost) / 100;
        }
        run_weeks(&mut w, &ds, 26);
        w.plans.migration.total[1]
    };
    let (small, big) = (flow(10), flow(30));
    assert!(
        big > small,
        "a gap of thirty percent pulls more than ten: {big} vs {small}"
    );
    assert!(small > 0);
    // Friction oracle: halved at 400 km, halved again over a state line.
    let n = w0.regions.len();
    let mut found_near = None;
    let mut found_far = None;
    for a in 0..n {
        for b in 0..n {
            if a == b {
                continue;
            }
            let km = ds.km(a as u8, b as u8) as i64;
            let f = migration::friction_bps(&w0, &ds, a, b);
            let same_state = w0.civics.state_of(a as u8) == w0.civics.state_of(b as u8);
            let mut expect = 10_000 * 400 / (400 + km);
            if !same_state {
                expect = expect * 5_000 / 10_000;
                if w0
                    .civics
                    .state_of(b as u8)
                    .and_then(|s| w0.civics.states.get(s))
                    .is_some_and(|s| s.laws.right_to_work)
                {
                    expect = expect * 8_300 / 10_000;
                }
            }
            assert_eq!(f, expect);
            if km < 100 && found_near.is_none() {
                found_near = Some(f);
            }
            if km > 800 && found_far.is_none() {
                found_far = Some(f);
            }
        }
    }
    if let (Some(near), Some(far)) = (found_near, found_far) {
        assert!(near > far);
    }
}
