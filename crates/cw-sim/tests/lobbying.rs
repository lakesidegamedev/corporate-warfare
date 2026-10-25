//! Bounded lobbying evidence (R24).
//!
//! Rules from docs/rules/FINANCE-LABOUR-AI.md: three request types, a fee of
//! max($25,000, 0.1 percent group assets) paid to the advocacy sector, one request per group
//! and region with a 26-week cooldown, a four-week decision, odds of
//! clamp(2000 + reputation/4 - recent wins * 500, 1000, 5000) bps drawn once and saved, costs
//! kept on failure, and a 250 bps public reputation loss for a failed campaign that coincides
//! with mass layoffs. PolicyKind has no variant that buys antitrust approval.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::{policy, seed};
use cw_sim::state::{HolderId, PolicyKind, ScenarioKind, ScenarioMode, ShareBook, ShareLot, World};

fn world(seed_value: u64) -> World {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Founder,
        ScenarioMode::Sandbox,
    );
    let all: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for (index, handle) in all.iter().enumerate() {
        register(
            &mut world,
            *handle,
            &[(HolderId::Outside(1_000 + index as u32), 1_000)],
        );
    }
    world
}

fn register(world: &mut World, firm: Handle, holders: &[(HolderId, i64)]) {
    let lots: BTreeMap<HolderId, ShareLot> = holders
        .iter()
        .map(|(holder, qty)| {
            (
                *holder,
                ShareLot {
                    qty: *qty,
                    cost_cents: 0,
                },
            )
        })
        .collect();
    world.firms.get_mut(firm).unwrap().shares = ShareBook::new(lots);
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

fn operative(world: &World) -> Vec<Handle> {
    world
        .firms
        .iter()
        .filter(|(_, firm)| firm.operative())
        .map(|(handle, _)| handle)
        .collect()
}

fn weeks(world: &mut World, count: u32) {
    for _ in 0..count {
        world.tick = world.tick.next();
        policy::weekly(world);
    }
}

/// First seed whose saved draw for this filing has the wanted outcome.
fn filed_with(outcome: bool, reputation: i32, kind: PolicyKind) -> (World, Handle, i64) {
    for seed_value in 1_000..1_400 {
        let mut world = world(seed_value);
        let firm = operative(&world)[0];
        world.firms.get_mut(firm).unwrap().reputation_bps = reputation;
        world.government.training_budget_cents = 1_000_000_000;
        fund(&mut world, firm, 100_000_000);
        let fee = policy::file_request(&mut world, firm, 2, kind).unwrap();
        if world.firm(firm).unwrap().policy_requests[0].success == outcome {
            return (world, firm, fee);
        }
    }
    panic!("no seed produced the wanted draw");
}

#[test]
fn odds_follow_reputation_and_recent_group_wins_within_bounds() {
    let mut world = world(1);
    let firm = operative(&world)[0];
    world.firms.get_mut(firm).unwrap().reputation_bps = 0;
    assert_eq!(policy::success_bps(&world, firm), 2_000);
    world.firms.get_mut(firm).unwrap().reputation_bps = 4_000;
    assert_eq!(policy::success_bps(&world, firm), 3_000);
    world.firms.get_mut(firm).unwrap().reputation_bps = 40_000;
    assert_eq!(
        policy::success_bps(&world, firm),
        5_000,
        "capped at 50 percent"
    );
    world.firms.get_mut(firm).unwrap().reputation_bps = -8_000;
    assert_eq!(
        policy::success_bps(&world, firm),
        1_000,
        "floored at 10 percent"
    );
}

#[test]
fn a_sibling_cannot_skip_its_groups_regional_cooldown() {
    let mut world = world(2);
    let firms = operative(&world);
    let (parent, first, second) = (firms[0], firms[1], firms[2]);
    register(&mut world, first, &[(HolderId::Firm(parent), 1_000)]);
    register(&mut world, second, &[(HolderId::Firm(parent), 1_000)]);
    for firm in [parent, first, second] {
        fund(&mut world, firm, 100_000_000);
    }
    policy::file_request(&mut world, first, 3, PolicyKind::PermitAcceleration).unwrap();
    assert!(
        policy::file_request(&mut world, second, 3, PolicyKind::TaxAdjustment).is_err(),
        "the sibling shares the parent's cooldown"
    );
    assert!(policy::file_request(&mut world, second, 4, PolicyKind::TaxAdjustment).is_ok());
    // Still cooling after the four-week decision; open again after 26 weeks.
    weeks(&mut world, 25);
    assert!(policy::file_request(&mut world, parent, 3, PolicyKind::TaxAdjustment).is_err());
    weeks(&mut world, 1);
    assert!(policy::file_request(&mut world, parent, 3, PolicyKind::TaxAdjustment).is_ok());
}

#[test]
fn the_fee_is_a_group_figure_with_a_floor() {
    let mut world = world(3);
    let firms = operative(&world);
    let (parent, child) = (firms[0], firms[1]);
    register(&mut world, child, &[(HolderId::Firm(parent), 1_000)]);
    assert!(policy::request_fee(&world, child) >= 2_500_000);
    let before = policy::request_fee(&world, child);
    // Adding $100m of parent cash adds 0.1 percent = $100,000 to the child's group fee.
    fund(&mut world, parent, 10_000_000_000);
    let after = policy::request_fee(&world, child);
    assert_eq!(after, policy::request_fee(&world, parent));
    assert!(after >= 10_000_000);
    if before > 2_500_000 {
        assert_eq!(after - before, 10_000_000);
    }
}

#[test]
fn a_failed_request_keeps_its_cost_and_changes_no_policy() {
    let (mut world, firm, fee) = filed_with(false, 0, PolicyKind::TaxAdjustment);
    let advocacy = world.outside.advocacy_book.balance(Account::Cash);
    assert!(advocacy >= fee, "the fee went to the advocacy sector");
    let cash = world.firm(firm).unwrap().book.cash();
    weeks(&mut world, 5);
    let request = &world.firm(firm).unwrap().policy_requests[0];
    assert!(request.resolved && !request.success);
    assert_eq!(world.regions[2].policy.tax_modifier_until, None);
    assert_eq!(world.firm(firm).unwrap().book.cash(), cash, "no refund");
    assert_eq!(world.outside.advocacy_book.balance(Account::Cash), advocacy);
}

#[test]
fn a_saved_draw_survives_a_save_and_reload() {
    let (world, firm, _) = filed_with(true, 12_000, PolicyKind::TaxAdjustment);
    let bytes = postcard::to_allocvec(&world).unwrap();
    let mut reloaded: World = postcard::from_bytes(&bytes).unwrap();
    let mut original = world;
    weeks(&mut original, 5);
    weeks(&mut reloaded, 5);
    for world in [&original, &reloaded] {
        let request = &world.firm(firm).unwrap().policy_requests[0];
        assert!(request.resolved && request.success);
        assert_eq!(world.regions[2].policy.tax_modifier_bps, -200);
    }
}

#[test]
fn a_won_training_grant_opens_the_subsidy_window_without_paying_cash() {
    let (mut world, firm, _) = filed_with(true, 12_000, PolicyKind::TrainingGrant);
    let cash = world.firm(firm).unwrap().book.cash();
    let budget = world.government.training_budget_cents;
    weeks(&mut world, 5);
    assert!(world.regions[2].policy.training_grant_until.is_some());
    assert_eq!(world.firm(firm).unwrap().book.cash(), cash);
    assert_eq!(world.government.training_budget_cents, budget);
}

#[test]
fn a_training_grant_without_a_government_budget_is_declined_before_the_fee() {
    let mut world = world(4);
    let firm = operative(&world)[0];
    fund(&mut world, firm, 100_000_000);
    world.government.training_budget_cents = 0;
    let cash = world.firm(firm).unwrap().book.cash();
    assert!(policy::file_request(&mut world, firm, 2, PolicyKind::TrainingGrant).is_err());
    assert_eq!(world.firm(firm).unwrap().book.cash(), cash);
}

#[test]
fn a_failed_campaign_during_mass_layoffs_costs_public_reputation() {
    let (mut world, firm, _) = filed_with(false, 0, PolicyKind::PermitAcceleration);
    let employed = world.firm(firm).unwrap().total_employees(&world.facilities);
    // Release a fifth of the workforce in the decision window.
    let released = (employed / 4).max(1);
    let week = world.tick.0 + 3;
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .layoffs
        .push((week, released));
    let reputation = world.firm(firm).unwrap().reputation_bps;
    weeks(&mut world, 5);
    assert_eq!(world.firm(firm).unwrap().reputation_bps, reputation - 250);
    assert!(
        world
            .news
            .iter()
            .any(|item| item.key == "news.lobbying_backlash"),
        "the penalty is recorded publicly"
    );
}

#[test]
fn a_failed_campaign_without_layoffs_costs_only_the_fee() {
    let (mut world, firm, _) = filed_with(false, 0, PolicyKind::PermitAcceleration);
    let reputation = world.firm(firm).unwrap().reputation_bps;
    weeks(&mut world, 5);
    assert_eq!(world.firm(firm).unwrap().reputation_bps, reputation);
}
