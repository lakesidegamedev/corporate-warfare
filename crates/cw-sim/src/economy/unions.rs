//! Worker organisation, bargaining and strikes.
//!
//! Organisation starts from the labelled regional figure. It rises 25 bps a week when pay is
//! below ninety percent of prevailing or when recent layoffs exceed ten percent, and otherwise
//! decays five bps to a floor of 500. At 5000 a ballot can form a bargaining unit. A demand is
//! made every 52 weeks: the wage floor is max(current * 1.05, prevailing * 1.02) plus one week
//! of severance for future layoffs. Two unresolved weeks start a strike: seventy percent of
//! affected workers are unavailable, striking workers are not paid, organisation rises 50 bps a
//! week, and after thirteen weeks a binding median settlement applies, capped by what the firm
//! can actually afford.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;

use crate::state::{BargainState, World};

pub const ORGANISATION_BALLOT_BPS: i32 = 5_000;
pub const STRIKE_DURATION_WEEKS: u32 = 13;

pub fn weekly(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let Some(firm) = world.firms.get(handle) else {
            continue;
        };
        if !firm.operative() {
            continue;
        }
        let region = firm.region;
        let prevailing = world.regions[region as usize].labour[0].prevailing_wage_cents;
        let wage = firm.policies.wage_offer[0];
        let entry = firm
            .organisation
            .get(&region)
            .copied()
            .unwrap_or(world.regions[region as usize].organisation_bps);
        let mut organisation = entry;
        if wage * 100 < prevailing * 90 {
            organisation += 25;
        } else {
            organisation = (organisation - 5).max(500);
        }
        if let Some(firm) = world.firms.get_mut(handle) {
            firm.organisation
                .insert(region, organisation.clamp(0, 10_000));
            firm.union.region = region;
            firm.union.organisation_bps = organisation.clamp(0, 10_000);
        }

        let state = world
            .firms
            .get(handle)
            .map(|firm| firm.union.state)
            .unwrap_or(BargainState::Idle);
        match state {
            BargainState::Idle => {
                let last = world
                    .firms
                    .get(handle)
                    .map(|firm| firm.union.last_demand_week)
                    .unwrap_or(0);
                let due = world.tick.0 >= last + 52;
                if organisation >= ORGANISATION_BALLOT_BPS && due && last > 0 {
                    begin_bargaining(world, handle, prevailing);
                }
            }
            BargainState::Demanding { deadline } => {
                if world.tick.0 >= deadline.0 {
                    if let Some(firm) = world.firms.get_mut(handle) {
                        firm.union.state = BargainState::Striking {
                            until: Tick(world.tick.0 + STRIKE_DURATION_WEEKS),
                            started: world.tick,
                        };
                    }
                    let region_state = &mut world.regions[region as usize];
                    region_state.demand_modifiers.push((
                        "strike".to_owned(),
                        None,
                        -500,
                        Tick(world.tick.0 + 4),
                    ));
                    world.news.push(crate::state::NewsItem {
                        week: world.tick,
                        key: "news.strike".to_owned(),
                        subject: world
                            .firms
                            .get(handle)
                            .map(|f| f.name.clone())
                            .unwrap_or_default(),
                        amount_cents: 0,
                        quantity: Default::default(),
                    });
                }
            }
            BargainState::Striking { until, .. } => {
                if let Some(firm) = world.firms.get_mut(handle) {
                    let org = (firm.union.organisation_bps + 50).min(10_000);
                    firm.union.organisation_bps = org;
                    firm.organisation.insert(region, org);
                }
                if world.tick.0 >= until.0 {
                    binding_settlement(world, handle);
                }
            }
            BargainState::Ballot => {
                // A successful ballot forms the bargaining unit; a failed one returns to the
                // ordinary organisation drift and waits for the next window.
                if organisation >= ORGANISATION_BALLOT_BPS {
                    begin_bargaining(world, handle, prevailing);
                } else if let Some(firm) = world.firms.get_mut(handle) {
                    firm.union.state = BargainState::Idle;
                }
            }
        }
        let _ = dataset;
    }
}

fn begin_bargaining(world: &mut World, firm: Handle, prevailing: i64) {
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let current = entity.policies.wage_offer[0].max(1);
    let demand = (current * 105 / 100).max(prevailing * 102 / 100);
    let affected = entity.total_employees(&world.facilities).max(1);
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.union.state = BargainState::Demanding {
            deadline: Tick(world.tick.0 + 2),
        };
        entity.union.demand_cents = demand;
        entity.union.last_demand_week = world.tick.0;
        entity.union.last_offer_cents = current;
        entity.union.settlement_cost_cents = demand * affected;
    }
}

/// Accept the union demand: raise the wage floor and pay one week of severance.
pub fn accept(world: &mut World, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let demand = entity.union.demand_cents.max(entity.policies.wage_offer[0]);
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.policies.wage_offer[0] = demand;
        entity.union.state = BargainState::Idle;
        entity.union.last_demand_week = world.tick.0;
    }
    demand
}

/// Counter at or above the current wage. Accepted when it reaches 98 percent of the demand
/// with a two-week no-layoff pledge.
pub fn counter(world: &mut World, firm: Handle, offer: i64) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    let demand = entity.union.demand_cents;
    let current = entity.policies.wage_offer[0];
    if offer < current {
        return false;
    }
    if offer * 100 >= demand * 98 {
        if let Some(entity) = world.firms.get_mut(firm) {
            entity.policies.wage_offer[0] = offer;
            entity.union.state = BargainState::Idle;
            entity.union.last_demand_week = world.tick.0;
        }
        return true;
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.union.last_offer_cents = offer;
        entity.union.state = BargainState::Demanding {
            deadline: Tick(world.tick.0 + 2),
        };
    }
    false
}

/// Reject: the two-week deadline continues to run toward a strike.
pub fn reject(world: &mut World, firm: Handle) {
    if let Some(entity) = world.firms.get_mut(firm)
        && let BargainState::Demanding { deadline } = entity.union.state
    {
        entity.union.state = BargainState::Demanding { deadline };
    }
}

/// Binding median settlement after thirteen weeks, capped by forecast affordability.
fn binding_settlement(world: &mut World, firm: Handle) {
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let demand = entity.union.demand_cents;
    let last_offer = entity.union.last_offer_cents;
    let median = (demand + last_offer) / 2;
    let workers = entity.total_employees(&world.facilities).max(1);
    let receipts: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.revenue)
        .sum::<i64>()
        / entity.book.recent_weeks().len().max(1) as i64;
    let non_labour = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.cost_of_goods + week.operating_expense)
        .sum::<i64>()
        / entity.book.recent_weeks().len().max(1) as i64;
    let affordable = ((receipts - non_labour) / workers).max(0);
    let contractual_floor = entity.policies.wage_offer[0];
    if affordable < contractual_floor {
        // No impossible automatic payment: record the failed settlement and let solvency run.
        world.failed_worker_settlements += 1;
        if let Some(entity) = world.firms.get_mut(firm) {
            entity.union.state = BargainState::Idle;
            entity.union.last_demand_week = world.tick.0;
        }
        return;
    }
    let settlement = median.min(affordable).max(contractual_floor);
    if settlement < demand {
        world.failed_worker_settlements += 1;
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.policies.wage_offer[0] = settlement;
        entity.union.state = BargainState::Idle;
        entity.union.last_demand_week = world.tick.0;
    }
}

/// Strike staffing multiplier in basis points: seventy percent unavailable.
pub fn staffing_multiplier_bps(world: &World, firm: Handle) -> i32 {
    match world.firms.get(firm).map(|f| f.union.state) {
        Some(BargainState::Striking { .. }) => 3_000,
        _ => 10_000,
    }
}

/// One week of severance for `workers` laid off, at the current wage floor.
pub fn severance_cost(world: &World, firm: Handle, workers: i64) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let wage = entity.union.demand_cents.max(entity.policies.wage_offer[0]);
    wage * workers.max(0)
}

/// Settlement payment posts through the ledger so a strike ending is never free.
pub fn settle_payment(world: &mut World, firm: Handle, amount: i64) {
    if amount <= 0 {
        return;
    }
    let cash = world
        .firms
        .get(firm)
        .map(|f| f.book.cash())
        .unwrap_or(0)
        .max(0);
    let paid = amount.min(cash);
    if paid > 0 {
        let _ = world.firms.get_mut(firm).unwrap().book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, paid, "union settlement")
                .debit(Account::Wages, paid, "union settlement"),
        );
    }
    if paid < amount {
        crate::economy::insolvency::defer_expense(
            world,
            firm,
            amount - paid,
            Account::Wages,
            "unpaid settlement",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn a_successful_ballot_forms_the_bargaining_unit() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 731, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let firm = world.investor.primary_group.unwrap();
        let region = world.firm(firm).unwrap().region;
        let prevailing = world.regions[region as usize].labour[0].prevailing_wage_cents;
        {
            let entity = world.firms.get_mut(firm).unwrap();
            entity.union.organisation_bps = 5_500;
            entity.union.ballot_support_bps = 5_500;
            entity.organisation.insert(region, 5_500);
            entity.union.state = BargainState::Ballot;
            entity.union.last_demand_week = 1;
            entity.policies.wage_offer[0] = prevailing * 90 / 100;
        }
        world.tick = Tick(60);
        weekly(&mut world, &dataset);
        assert!(matches!(
            world.firm(firm).unwrap().union.state,
            BargainState::Demanding { .. }
        ));
        let demand = world.firm(firm).unwrap().union.demand_cents;
        assert!(
            demand >= prevailing * 102 / 100,
            "the demand follows the wage rule"
        );
    }

    #[test]
    fn a_failed_ballot_returns_to_idle() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 732, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let firm = world.investor.primary_group.unwrap();
        {
            let entity = world.firms.get_mut(firm).unwrap();
            entity.organisation.insert(entity.region, 4_000);
            entity.union.organisation_bps = 4_000;
            entity.union.ballot_support_bps = 4_000;
            entity.union.state = BargainState::Ballot;
        }
        weekly(&mut world, &dataset);
        assert!(matches!(
            world.firm(firm).unwrap().union.state,
            BargainState::Idle
        ));
    }
}
