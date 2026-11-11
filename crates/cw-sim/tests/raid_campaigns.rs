//! Raid campaigns (2.0): announced weeks ahead with the budget in escrow, decided against the
//! defence at resolution, lost if abandoned; and the whole-business stake rule. Expected values
//! are worked by hand from the rule stated on `sectors::whole_business_chance`.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::{sectors, seed};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};

fn fund(world: &mut World, firm: Handle, cents: i64) {
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test funding")
            .credit(Account::ShareCapital, cents, "test funding"),
    );
}

/// All money: every firm's cash and restricted cash plus the outside accounts a raid pays.
fn money(world: &World) -> i64 {
    world
        .firms
        .iter()
        .map(|(_, f)| f.book.cash() + f.book.balance(Account::RestrictedCash))
        .sum::<i64>()
        + world.outside.gateway_book.cash()
        + world.outside.legal_services_book.cash()
}

struct Setup {
    world: World,
    dataset: Dataset,
    attacker: Handle,
    defender: Handle,
    region: u8,
    industry: u8,
    /// The defender's floor defence with no marketing.
    defence: i64,
}

/// An attacker with a farm, and a defender whose whole business is one farm of `blocks` blocks
/// in the same cell, past its grace, both with no marketing.
fn setup(blocks: u8) -> Setup {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 902, ScenarioKind::Founder, ScenarioMode::Sandbox);
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
    for old in world.firms.get(defender).unwrap().facilities.clone() {
        world.facilities.remove(old);
    }
    let mut farm = world.facilities.get(plant).unwrap().clone();
    farm.firm = defender;
    farm.blocks = blocks;
    farm.capex_cents = 2_000_000 * blocks as i64;
    farm.accumulated_depreciation = 0;
    farm.started = Tick(0);
    let farm = world.facilities.insert(farm);
    world.firms.get_mut(defender).unwrap().facilities = vec![farm];
    for firm in [attacker, defender] {
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .policies
            .marketing_budget_cents = 0;
    }
    fund(&mut world, attacker, 10_000_000_000);
    world.tick = Tick(sectors::RAID_GRACE_WEEKS + 1);
    let template = &dataset.recipes[recipe as usize];
    let price = dataset.reference_price(dataset.product_index[template.output.as_str()]);
    let defence = template.weekly_output_lots * blocks as i64 * price * 13 * 500 / 10_000;
    Setup {
        world,
        dataset,
        attacker,
        defender,
        region,
        industry,
        defence,
    }
}

#[test]
fn the_whole_business_rule_worked_by_hand() {
    // chance = c x (1 - s x (1 - b)), floor 5 percent x (1 - s), cap 95 percent.
    // Attack three times the defence: b = 75 percent; a one block division, c = 11.25 percent.
    // Its company's only block (s = 1): 11.25 x (1 - 0.25) = 8.4375, rounded down.
    assert_eq!(sectors::whole_business_chance(1_125, 7_500, 10_000), 843);
    // One block of a ten block company (s = 0.1): 11.25 x (1 - 0.1 x 0.25) = 10.96875.
    assert_eq!(sectors::whole_business_chance(1_125, 7_500, 1_000), 1_096);
    // Even strength (b = 50 percent, c = 7.5): the only block 3.75 percent, no floor.
    assert_eq!(sectors::whole_business_chance(750, 5_000, 10_000), 375);
    // A tiny share keeps the old 5 percent floor: 3 x (1 - 0) = 3, floored at 5.
    assert_eq!(sectors::whole_business_chance(300, 5_000, 0), 500);
    // Half the business at stake halves the floor: 1 x (1 - 0.5 x 0.5) = 0.75, floor 2.5.
    assert_eq!(sectors::whole_business_chance(100, 5_000, 5_000), 250);
}

#[test]
fn a_companys_only_block_is_quoted_at_the_whole_business_chance() {
    let s = setup(1);
    // No marketing: defence is the floor; the attacker has no marketing either, so the attack
    // is the budget. Three to one: base 75 percent, stake 1 block of 1 (division share 100
    // percent drags it to 11.25), whole-business share 100 percent: 8.43 percent (was 11.25).
    let quote = sectors::raid_quote(
        &s.world,
        &s.dataset,
        s.attacker,
        s.defender,
        s.region,
        s.industry,
        s.defence * 3,
    );
    assert_eq!(quote.defence_cents, s.defence);
    assert_eq!(quote.blocks_at_stake, 1);
    assert_eq!(quote.chance_bps, 843);
}

#[test]
fn a_campaign_escrows_its_budget_and_is_decided_against_the_defence_at_resolution() {
    let mut s = setup(6);
    let warning = sectors::raid_warning_weeks(&s.dataset);
    assert_eq!(warning, 4, "content sets a four week warning");
    let budget = s.defence * 2;
    let total = money(&s.world);
    let cash_before = s.world.firm(s.attacker).unwrap().book.cash();
    let campaign = sectors::announce_raid(
        &mut s.world,
        &s.dataset,
        s.attacker,
        s.defender,
        s.region,
        s.industry,
        budget,
    )
    .expect("announced");
    assert_eq!(campaign.resolves, s.world.tick.0 + 4);
    let book = &s.world.firm(s.attacker).unwrap().book;
    assert_eq!(book.cash(), cash_before - budget);
    assert_eq!(book.balance(Account::RestrictedCash), budget);
    assert!(
        s.world.raids.is_empty(),
        "nothing is decided at announcement"
    );
    assert_eq!(money(&s.world), total);
    // One campaign per raider on the same division at a time.
    assert!(
        sectors::announce_raid(
            &mut s.world,
            &s.dataset,
            s.attacker,
            s.defender,
            s.region,
            s.industry,
            budget,
        )
        .is_err()
    );
    let announced = sectors::raid_quote(
        &s.world, &s.dataset, s.attacker, s.defender, s.region, s.industry, budget,
    );
    // Not due yet: three weeks on, still pending.
    s.world.tick.0 += 3;
    sectors::resolve_raid_campaigns(&mut s.world, &s.dataset);
    assert_eq!(s.world.raids.campaigns.len(), 1);
    // The defender doubles its defence during the warning: weekly marketing of a thirteenth of
    // the floor adds a quarter's worth equal to the floor.
    s.world
        .firms
        .get_mut(s.defender)
        .unwrap()
        .policies
        .marketing_budget_cents = s.defence / 13;
    s.world.tick.0 += 1;
    let at_resolution = sectors::raid_quote(
        &s.world, &s.dataset, s.attacker, s.defender, s.region, s.industry, budget,
    );
    assert!(at_resolution.chance_bps < announced.chance_bps);
    sectors::resolve_raid_campaigns(&mut s.world, &s.dataset);
    assert!(s.world.raids.campaigns.is_empty());
    let record = s.world.raids.last().expect("decided").clone();
    assert_eq!(record.tick, campaign.resolves);
    assert_eq!(record.budget_cents, budget);
    assert_eq!(
        record.chance_bps, at_resolution.chance_bps,
        "the defence added during the warning counts"
    );
    assert_eq!(
        s.world
            .firm(s.attacker)
            .unwrap()
            .book
            .balance(Account::RestrictedCash),
        0
    );
    assert_eq!(
        money(&s.world),
        total,
        "a campaign moves money, never mints it"
    );
}

#[test]
fn an_abandoned_campaign_loses_its_budget_and_takes_nothing() {
    let mut s = setup(4);
    let budget = s.defence * 4;
    let total = money(&s.world);
    let cash_before = s.world.firm(s.attacker).unwrap().book.cash();
    sectors::announce_raid(
        &mut s.world,
        &s.dataset,
        s.attacker,
        s.defender,
        s.region,
        s.industry,
        budget,
    )
    .unwrap();
    s.world.tick.0 += 2;
    sectors::abandon_raid(
        &mut s.world,
        &s.dataset,
        s.attacker,
        s.defender,
        s.region,
        s.industry,
    )
    .unwrap();
    assert!(s.world.raids.campaigns.is_empty());
    assert!(
        s.world.raids.is_empty(),
        "an abandoned campaign decides nothing"
    );
    let book = &s.world.firm(s.attacker).unwrap().book;
    assert_eq!(book.balance(Account::RestrictedCash), 0);
    assert_eq!(
        book.cash(),
        cash_before - budget,
        "the budget is not returned"
    );
    assert_eq!(money(&s.world), total);
    // Nothing left to abandon.
    assert!(
        sectors::abandon_raid(
            &mut s.world,
            &s.dataset,
            s.attacker,
            s.defender,
            s.region,
            s.industry,
        )
        .is_err()
    );
}
