//! Executives (1.9): each trait moves its one number by exactly the stated amount, rivals hire
//! only when it pays, poaching follows its rule, and the board replaces a player's chief after
//! two missed mandates in a row.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::{Dataset, Temperament};
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::economy::chapters::{ChapterLog, Outcome};
use cw_sim::economy::executives::{self as ex, Post};
use cw_sim::economy::seed;
use cw_sim::state::{ScenarioKind, ScenarioMode, World};

fn setup(seed_value: u64) -> (World, Dataset, Handle) {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Guided,
        ScenarioMode::Sandbox,
    );
    ex::weekly(&mut world, &dataset);
    let player = world.investor.primary_group.unwrap();
    (world, dataset, player)
}

fn fund(world: &mut World, firm: Handle, cents: i64) {
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, cents, "fixture funding")
                .credit(Account::ShareCapital, cents, "fixture funding"),
        )
        .unwrap();
}

fn envelope(world: &World, actor: u64, intent: CommandIntent) -> CommandEnvelope {
    CommandEnvelope {
        command_id_high: 7,
        command_id_low: world.revision,
        actor,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent,
    }
}

/// Put a person of the given level into a post, by the hire command so pay and fees are real.
fn seat(world: &mut World, ds: &Dataset, firm: Handle, post: Post, level: u8) -> u16 {
    fund(world, firm, 50_000_000_000);
    let id = ex::candidates(world, post)
        .first()
        .map(|c| c.id)
        .expect("the pool has candidates");
    world
        .executives
        .people
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .level = level;
    let command = envelope(
        world,
        0,
        CommandIntent::HireExecutive {
            firm,
            post: post.code(),
            person: id,
        },
    );
    cw_sim::commands::apply(world, ds, &command).expect("hire");
    id
}

/// Put a person of the given level straight into a post, with no fee and no cash moving, so an
/// effect can be read against an otherwise identical world.
fn place(world: &mut World, firm: Handle, post: Post, level: u8) -> u16 {
    let id = ex::candidates(world, post).first().map(|c| c.id).unwrap();
    world
        .executives
        .people
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .level = level;
    world.executives.posts.push(ex::Executive {
        firm,
        post,
        person: id,
        since: world.tick.0,
        expires: world.tick.0 + 104,
        pay_cents: 1,
    });
    id
}

#[test]
fn the_pool_is_drawn_once_with_levels_one_to_five_for_every_post() {
    let (world, _, _) = setup(11);
    for post in Post::ALL {
        let list = ex::candidates(&world, post);
        assert!(list.len() >= 8, "{post:?} has candidates");
        assert!(list.iter().all(|c| (1..=5).contains(&c.level)));
        assert!(list.iter().all(|c| ex::person_of(c).is_some()));
    }
    let mut extras: Vec<u16> = world.executives.people.iter().map(|c| c.extra).collect();
    extras.sort_unstable();
    extras.dedup();
    assert_eq!(extras.len(), world.executives.people.len(), "no one twice");
}

#[test]
fn a_cfo_lowers_loan_and_bond_spreads_by_fifteen_points_a_level() {
    let (mut world, _ds, firm) = setup(12);
    let grade = world.firm(firm).unwrap().grade.spread_bps();
    let bond_before =
        cw_sim::economy::structure::bond_quote(&world, firm, 100_000_000, 260).spread_bps;
    assert_eq!(ex::spread_after_cut(&world, firm, grade), grade);
    place(&mut world, firm, Post::Cfo, 5);
    assert_eq!(ex::spread_after_cut(&world, firm, grade), grade - 75);
    let bond_after =
        cw_sim::economy::structure::bond_quote(&world, firm, 100_000_000, 260).spread_bps;
    assert_eq!(bond_before - bond_after, 75);
    for level in 1..=5u8 {
        assert_eq!(ex::spread_cut_for(level), 15 * level as i32);
    }
}

#[test]
fn a_loan_priced_below_the_old_quote_is_refused_until_a_cfo_is_hired() {
    let (mut world, ds, firm) = setup(13);
    let old = world.macro_state.policy_rate_bps
        + cw_sim::economy::macro_cycle::lending_premium_bps(&world)
        + world.firm(firm).unwrap().grade.spread_bps();
    let ask = |world: &World| {
        envelope(
            world,
            0,
            CommandIntent::AcceptLoanQuote {
                firm,
                amount_cents: 1_000_000,
                rate_bps: old - 75,
                weeks: 52,
            },
        )
    };
    let mut plain = world.clone();
    let refusal = ask(&plain);
    assert!(cw_sim::commands::apply(&mut plain, &ds, &refusal).is_err());
    seat(&mut world, &ds, firm, Post::Cfo, 5);
    let command = ask(&world);
    cw_sim::commands::apply(&mut world, &ds, &command).expect("the CFO's rate is accepted");
}

#[test]
fn a_cfo_earns_interest_on_cash_at_four_points_a_level() {
    let (mut world, ds, firm) = setup(14);
    seat(&mut world, &ds, firm, Post::Cfo, 5);
    let cash = world.firm(firm).unwrap().book.cash();
    let income_before = world
        .firm(firm)
        .unwrap()
        .book
        .balance(Account::InterestIncome);
    world.tick = Tick(world.tick.0 + 1);
    // Only the interest step: pay is separate, so read the income account.
    ex::weekly(&mut world, &ds);
    let income = income_before
        - world
            .firm(firm)
            .unwrap()
            .book
            .balance(Account::InterestIncome);
    // Interest is on the cash after the week's pay.
    let expected_high = cash * 20 / (10_000 * 52);
    assert!(
        income > 0 && income <= expected_high,
        "{income} vs {expected_high}"
    );
    assert_eq!(ex::cash_yield_for(5), 20);
}

#[test]
fn a_coo_cuts_build_weeks_by_four_percent_a_level() {
    let (mut world, _ds, firm) = setup(15);
    assert_eq!(ex::build_weeks(&world, firm, 50), 50);
    let id = place(&mut world, firm, Post::Coo, 5);
    assert_eq!(ex::build_weeks(&world, firm, 50), 40);
    world
        .executives
        .people
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .level = 1;
    assert_eq!(ex::build_weeks(&world, firm, 50), 48);
    assert_eq!(ex::build_weeks(&world, firm, 1), 1, "never below a week");
}

#[test]
fn a_coo_lets_a_full_plant_run_eight_tenths_of_a_point_a_level_above_rating() {
    let (world, ds, firm) = setup(16);
    let mut with = world.clone();
    place(&mut with, firm, Post::Coo, 5);
    let plants: Vec<Handle> = world.firm(firm).unwrap().facilities.clone();
    let mut checked = 0;
    for plant in plants {
        let mut base = world.clone();
        let mut boosted = with.clone();
        // Same state, same week, only the executive differs.
        boosted.executives.posts.retain(|e| e.post == Post::Coo);
        cw_sim::economy::production::produce(&mut base, &ds);
        cw_sim::economy::production::produce(&mut boosted, &ds);
        let a = base.facilities.get(plant).unwrap().last_output.0;
        let b = boosted.facilities.get(plant).unwrap().last_output.0;
        if b > a {
            // Four percent more output (level 5), to rounding of the input scaling.
            assert!((b - a * 10_400 / 10_000).abs() <= 2, "{b} vs {a}");
            checked += 1;
        } else {
            assert_eq!(b, a, "a plant below its rating is unchanged");
        }
    }
    assert!(checked > 0, "at least one plant ran flat out and rose");
}

#[test]
fn a_logistics_chief_cuts_the_outside_premium_by_five_percent_a_level() {
    let (mut world, ds, firm) = setup(17);
    let mult = ds.logistics.outside_premium_bps;
    let empty = ds.logistics.empty_lane_bps;
    assert_eq!(ex::outside_mult_bps(&world, &ds, firm, mult), mult);
    seat(&mut world, &ds, firm, Post::Logistics, 5);
    let cut = ex::outside_mult_bps(&world, &ds, firm, mult);
    assert_eq!(cut, empty + (mult - empty) * 75 / 100);
    assert!(cut >= empty, "never below the owners' empty-lane rate");
    assert_eq!(ex::lane_fee_after_cut(&world, firm, 10_000), 9_500);
}

#[test]
fn pay_grows_with_level_and_with_company_size() {
    let small = 400_000_000;
    let large = 40_000_000_000;
    for level in 1..5u8 {
        assert!(ex::pay_for(level, small) < ex::pay_for(level + 1, small));
        assert!(ex::pay_for(level, small) < ex::pay_for(level, large));
    }
    // A company a hundred times larger pays ten times more: pay follows the square root.
    assert_eq!(ex::pay_for(3, large), ex::pay_for(3, small) * 10);
}

#[test]
fn contracts_run_two_years_and_dismissal_pays_severance() {
    let (mut world, ds, firm) = setup(18);
    let start = world.tick.0;
    seat(&mut world, &ds, firm, Post::Coo, 3);
    let e = world.executives.holder(firm, Post::Coo).unwrap().clone();
    assert_eq!(e.expires, start + 104);
    let cash = world.firm(firm).unwrap().book.cash();
    let severance = ex::severance_cents(&world, &e);
    assert!(severance >= e.pay_cents * 4 && severance <= e.pay_cents * 26);
    let command = envelope(
        &world,
        0,
        CommandIntent::DismissExecutive {
            firm,
            post: Post::Coo.code(),
        },
    );
    cw_sim::commands::apply(&mut world, &ds, &command).unwrap();
    assert_eq!(world.firm(firm).unwrap().book.cash(), cash - severance);
    assert!(world.executives.holder(firm, Post::Coo).is_none());
    assert!(world.firm(firm).unwrap().book.is_balanced());
    assert!(world.outside.gateway_book.is_balanced());
}

#[test]
fn an_unpaid_executive_leaves_and_an_expired_contract_renews_or_ends() {
    let (mut world, ds, firm) = setup(19);
    seat(&mut world, &ds, firm, Post::Logistics, 2);
    let expires = world
        .executives
        .holder(firm, Post::Logistics)
        .unwrap()
        .expires;
    world.tick = Tick(expires);
    ex::weekly(&mut world, &ds);
    let kinds: Vec<_> = world.executives.events.iter().map(|e| e.kind).collect();
    assert!(
        kinds.contains(&ex::EventKind::Renewed) || kinds.contains(&ex::EventKind::Left),
        "{kinds:?}"
    );
}

#[test]
fn a_rival_hires_only_when_the_effect_pays_for_the_pay() {
    let (mut world, ds, _) = setup(20);
    let rival = world
        .firms
        .iter()
        .find(|(h, f)| {
            f.archetype.is_some() && f.operative() && Some(*h) != world.investor.primary_group
        })
        .map(|(h, _)| h)
        .unwrap();
    world.tick = Tick(4);
    // No debt and little cash: nothing a CFO does is worth his pay.
    let quiet = ex::rival_pick(&world, &ds, rival);
    if let Some((post, person)) = quiet {
        let c = world.executives.person(person).unwrap();
        let value = ex::yearly_value_cents(&world, &ds, rival, post, c.level);
        assert!(value * 2 >= ex::ask_cents(&world, rival, c) * 52 * 3);
    }
    // A heavily borrowed, cash-rich firm: a chief financial officer pays.
    fund(&mut world, rival, 400_000_000_000);
    {
        let f = world.firms.get_mut(rival).unwrap();
        f.book
            .commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, 100_000_000_000, "borrowing")
                    .credit(Account::Loans, 100_000_000_000, "borrowing"),
            )
            .unwrap();
    }
    let (post, person) = ex::rival_pick(&world, &ds, rival).expect("a borrower hires");
    assert_eq!(post, Post::Cfo);
    let c = world.executives.person(person).unwrap();
    let value = ex::yearly_value_cents(&world, &ds, rival, post, c.level);
    assert!(value * 2 >= ex::ask_cents(&world, rival, c) * 52 * 3);
    // The rival's hire is the same command a player sends and passes the same checks.
    let command = envelope(
        &world,
        rival.0,
        CommandIntent::HireExecutive {
            firm: rival,
            post: post.code(),
            person,
        },
    );
    // A rival is not under the investor's control, but its own actor handle is.
    cw_sim::commands::apply(&mut world, &ds, &command).expect("rival hire");
    assert!(world.executives.holder(rival, Post::Cfo).is_some());
    // A firm that cannot cover twenty-six weeks of pay does not hire.
    let (mut poor, ds2, _) = setup(20);
    poor.tick = Tick(4);
    let poor_rival = poor
        .firms
        .iter()
        .find(|(h, f)| {
            f.archetype.is_some() && f.operative() && Some(*h) != poor.investor.primary_group
        })
        .map(|(h, _)| h)
        .unwrap();
    let cash = poor.firm(poor_rival).unwrap().book.cash();
    poor.firms
        .get_mut(poor_rival)
        .unwrap()
        .book
        .commit(
            poor.tick,
            Txn::new()
                .debit(Account::RetainedEarnings, cash, "drain")
                .credit(Account::Cash, cash, "drain"),
        )
        .unwrap();
    assert!(ex::rival_pick(&poor, &ds2, poor_rival).is_none());
}

#[test]
fn poaching_is_accepted_only_at_the_temperaments_price() {
    let (mut world, ds, player) = setup(21);
    let rival = world
        .firms
        .iter()
        .find(|(h, f)| f.archetype.is_some() && f.operative() && *h != player)
        .map(|(h, _)| h)
        .unwrap();
    place(&mut world, rival, Post::Cfo, 4);
    world.executives.posts.last_mut().unwrap().pay_cents = 400_000;
    fund(&mut world, player, 50_000_000_000);
    let person = world.executives.holder(rival, Post::Cfo).unwrap().person;
    let pay = world.executives.holder(rival, Post::Cfo).unwrap().pay_cents;
    let t = ex::temperament_of(world.executives.person(person).unwrap());
    let price = ex::poach_price_cents(&world, rival, Post::Cfo).unwrap();
    assert_eq!(
        price,
        pay * (10_000 + ex::poach_hurdle_bps(t)) as i64 / 10_000
    );
    // A cautious person wants more than a speculator.
    assert!(
        ex::poach_hurdle_bps(Temperament::Cautious) > ex::poach_hurdle_bps(Temperament::Speculator)
    );
    let low = envelope(
        &world,
        0,
        CommandIntent::PoachExecutive {
            firm: player,
            from: rival,
            post: Post::Cfo.code(),
            offer_cents: price - 1,
        },
    );
    let mut refused = world.clone();
    assert!(cw_sim::commands::apply(&mut refused, &ds, &low).is_err());
    assert!(refused.executives.holder(rival, Post::Cfo).is_some());
    let fair = envelope(
        &world,
        0,
        CommandIntent::PoachExecutive {
            firm: player,
            from: rival,
            post: Post::Cfo.code(),
            offer_cents: price,
        },
    );
    cw_sim::commands::apply(&mut world, &ds, &fair).expect("accepted at the price");
    assert!(world.executives.holder(rival, Post::Cfo).is_none());
    let moved = world.executives.holder(player, Post::Cfo).unwrap();
    assert_eq!(moved.person, person);
    assert_eq!(moved.pay_cents, price);
    assert!(
        world
            .executives
            .events
            .iter()
            .any(|e| e.kind == ex::EventKind::Poached
                && e.firm == rival
                && e.other == Some(player))
    );
    // The player cannot take their own executive, and cannot fill a filled post.
    let again = envelope(
        &world,
        0,
        CommandIntent::PoachExecutive {
            firm: player,
            from: player,
            post: Post::Cfo.code(),
            offer_cents: price * 2,
        },
    );
    assert!(cw_sim::commands::apply(&mut world, &ds, &again).is_err());
}

#[test]
fn two_missed_mandates_in_a_row_bring_in_a_chief_from_the_board_once() {
    let (mut world, ds, player) = setup(22);
    assert!(world.firm(player).unwrap().ceo.is_none());
    world.tick = Tick(105);
    for (number, start) in [(1u32, 1u32), (2, 53)] {
        world.chapters.log.push(ChapterLog {
            number,
            name: format!("Chapter {number}"),
            start,
            firm: Some(player),
            offered: vec![],
            chosen: Some(0),
            outcome: Some(Outcome::Missed),
            closed: if number == 2 { 105 } else { 53 },
            bonus_cents: 0,
            reputation_bps: 0,
        });
    }
    ex::weekly(&mut world, &ds);
    let chief = world
        .firm(player)
        .unwrap()
        .ceo
        .expect("the board named a chief");
    assert!(
        world
            .executives
            .events
            .iter()
            .any(|e| e.kind == ex::EventKind::BoardChief && e.firm == player)
    );
    assert_eq!(ex::board_chapter(&world, player), 2);
    // The same week again does nothing more.
    ex::weekly(&mut world, &ds);
    assert_eq!(world.firm(player).unwrap().ceo, Some(chief));
    assert_eq!(
        world
            .executives
            .events
            .iter()
            .filter(|e| e.kind == ex::EventKind::BoardChief)
            .count(),
        1
    );
}

#[test]
fn one_missed_mandate_or_a_met_one_between_does_not_replace_the_chief() {
    let (mut world, ds, player) = setup(23);
    world.tick = Tick(105);
    for (number, outcome) in [
        (1u32, Outcome::Missed),
        (2, Outcome::Met),
        (3, Outcome::Missed),
    ] {
        world.chapters.log.push(ChapterLog {
            number,
            name: String::new(),
            start: 1 + 52 * (number - 1),
            firm: Some(player),
            offered: vec![],
            chosen: Some(0),
            outcome: Some(outcome),
            closed: if number == 3 { 105 } else { 1 },
            bonus_cents: 0,
            reputation_bps: 0,
        });
    }
    ex::weekly(&mut world, &ds);
    assert!(world.firm(player).unwrap().ceo.is_none());
}

#[test]
fn the_state_hashes_and_replays_the_same() {
    let (mut a, ds, firm) = setup(24);
    seat(&mut a, &ds, firm, Post::Cfo, 3);
    let b = a.clone();
    assert_eq!(
        cw_sim::state::canonical_hash(&a),
        cw_sim::state::canonical_hash(&b)
    );
    let mut empty = b.clone();
    empty.executives = Default::default();
    assert_ne!(
        cw_sim::state::canonical_hash(&a),
        cw_sim::state::canonical_hash(&empty),
        "executives are future-affecting and hashed"
    );
}

#[test]
fn the_scenarios_named_people_for_hire_are_drawn_before_generated_ones() {
    use cw_data::cast::{NAMED_HIRE_BASE, named_hires};
    for scenario in [
        "texas-1981",
        "ruhr-rhine-1991",
        "silicon-valley-1995",
        "great-lakes-2000",
    ] {
        let dataset = Dataset::load_scenario(scenario).unwrap();
        let mut world = seed::build(&dataset, 5, ScenarioKind::Guided, ScenarioMode::Sandbox);
        world.cast.scenario_id = scenario.to_owned();
        world.executives = Default::default();
        ex::weekly(&mut world, &dataset);
        let named: Vec<_> = world
            .executives
            .people
            .iter()
            .filter(|c| c.extra >= NAMED_HIRE_BASE)
            .collect();
        assert_eq!(named.len(), 2, "{scenario}: two named people for hire");
        for c in &named {
            let hire = ex::hire_of(c).unwrap();
            assert!(hire.named && hire.person.scenario == scenario, "{scenario}");
            assert!(!hire.person.hire_note.is_empty());
            assert_eq!(ex::portrait_id(c), hire.person.id);
        }
        // Named people sit at the head of the pool: lower ids than any generated candidate of
        // their post, and the pool is otherwise the usual size.
        for c in &named {
            let first_generated = world
                .executives
                .people
                .iter()
                .filter(|g| g.post == c.post && g.extra < NAMED_HIRE_BASE)
                .map(|g| g.id)
                .min()
                .unwrap();
            assert!(c.id < first_generated, "{scenario}");
        }
        assert_eq!(named_hires().len(), 8);
        // A world with no scenario draws only generated people.
        world.cast.scenario_id.clear();
        world.executives = Default::default();
        ex::weekly(&mut world, &dataset);
        assert!(world.executives.people.iter().all(|c| c.extra < NAMED_HIRE_BASE));
    }
}
