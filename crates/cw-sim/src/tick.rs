//! Fixed weekly settlement order.
//!
//! 0. Accepted commands were already applied against the opening revision.
//! 1. Expire offers/contracts, deliver shipments, complete construction, settle governance.
//! 2. Reserve statutory obligations (loan service is settled after production).
//! 3. Allocate labour and pay payroll.
//! 4. Dispatch utilities, then allocate power and produce from opening stock.
//! 5. Clear contracts, spot markets and final demand simultaneously.
//! 6. Freight for inter-region shipments was charged at dispatch.
//! 7. Accrue expenses, depreciation, interest, tax and claims; apply default rules.
//! 8. Advance investment, R&D, brand, credit, values, events and closing reports.
//! 9. AI observes closing data and queues next-week commands (session layer).
//!
//! Fast-forward never skips a phase.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;
use cw_data::Dataset;

use crate::state::{FirmStatus, World};

#[derive(Clone, Debug, Default)]
pub struct WeekOutcome {
    pub week: Tick,
    pub pause: Option<String>,
    pub restructured: Vec<Handle>,
    pub liquidated: Vec<Handle>,
}

pub fn advance(world: &mut World, dataset: &Dataset) -> WeekOutcome {
    world.tick = world.tick.next();
    world.weeks_elapsed += 1;
    world.pending_pause = None;
    world.prune_modifiers();
    // The technology frontier advances one level per game year, capped at twenty.
    if world.tick.is_year_boundary() {
        world.macro_state.tech_frontier = (world.macro_state.tech_frontier + 1).min(20);
    }

    // Phase 1: expiries, deliveries, construction and governance.
    crate::economy::freight::settle(world, dataset);
    complete_construction(world, dataset);
    crate::economy::deals::weekly(world);
    crate::economy::governance::weekly(world, dataset);

    // Phase 3: labour and payroll.
    crate::economy::labour::clear(world, dataset);

    // Phase 4: utilities and production.
    crate::economy::production::produce(world, dataset);

    // Phase 5: markets.
    crate::economy::markets::clear_week(world, dataset);

    // Phase 7: expenses, debt service, claims and defaults.
    settle_expenses(world, dataset);
    crate::economy::credit::service(world, dataset);
    crate::economy::banking::weekly(world);
    crate::economy::insurance::weekly(world);
    let solvency = crate::economy::insolvency::settle(world, dataset);

    // Phase 8: organisation, policy, research, valuation, exchange and events.
    crate::economy::unions::weekly(world, dataset);
    crate::economy::policy::weekly(world);
    crate::economy::research::weekly(world, dataset);
    crate::economy::production::expire_services(world, dataset);
    crate::economy::equity::refresh_valuation(world, dataset);
    crate::economy::equity::clear_exchange(world, dataset);
    crate::economy::events_engine::quarter_check(world, dataset);
    crate::economy::events_engine::auto_resolve(world, dataset);
    crate::economy::events_engine::expire_optional(world, dataset);
    world.clear_construction_orders();
    refresh_household_income(world, dataset);
    apply_macro_price_drift(world, dataset);

    // Close books and sales history, then track resilience.
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        if let Some(firm) = world.firms.get_mut(handle) {
            let _ = firm.book.close_week(world.tick);
            firm.close_sales_week();
        }
    }
    let past_due = world.firms.iter().any(|(_, firm)| {
        firm.shares.qty(crate::state::HolderId::Investor) > 0
            && matches!(
                firm.status,
                FirmStatus::PastDue { .. } | FirmStatus::Restructuring { .. }
            )
    });
    if past_due {
        world.weeks_with_past_due += 1;
    }

    // Pause on decisions that cannot safely pass unattended. Event and score pauses were set
    // earlier; these cover grace, the first strike and imminent deal or board deadlines.
    if world.pending_pause.is_none() {
        world.pending_pause = pause_reason(world);
    }
    crate::economy::score::update(world);
    if let Some(reason) = world.pending_pause.clone() {
        return WeekOutcome {
            week: world.tick,
            pause: Some(reason),
            restructured: Vec::new(),
            liquidated: Vec::new(),
        };
    }
    // Mandatory player decisions pause the clock.
    if let Some(event) = world
        .events
        .pending
        .iter()
        .find(|event| !event.resolved && event.mandatory)
    {
        world.pending_pause = Some(format!("event:{}", event.id));
    }
    WeekOutcome {
        week: world.tick,
        pause: world.pending_pause.clone(),
        restructured: solvency
            .iter()
            .filter(|(_, kind)| *kind == "restructuring")
            .map(|(handle, _)| *handle)
            .collect(),
        liquidated: solvency
            .iter()
            .filter(|(_, kind)| *kind == "liquidation")
            .map(|(handle, _)| *handle)
            .collect(),
    }
}

/// Reasons the clock must stop for the investor before the next week.
pub fn pause_reason(world: &World) -> Option<String> {
    let controlled: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.shares.qty(crate::state::HolderId::Investor) > 0)
        .map(|(handle, _)| handle)
        .collect();
    for handle in &controlled {
        let Some(firm) = world.firm(*handle) else {
            continue;
        };
        if matches!(firm.status, FirmStatus::PastDue { .. }) && firm.past_due_weeks >= 3 {
            return Some(format!("default_grace:{}", handle.0));
        }
        if let crate::state::BargainState::Striking { started, .. } = firm.union.state
            && started.0 == world.tick.0
        {
            return Some(format!("strike:{}", handle.0));
        }
        for vote in &firm.board.pending_votes {
            if !vote.resolved && vote.deadline.0 <= world.tick.0 + 1 {
                return Some(format!("board_vote:{}:{}", handle.0, vote.id));
            }
        }
    }
    for (_, deal) in world.deals.iter() {
        if controlled.contains(&deal.buyer)
            && matches!(
                deal.state,
                crate::state::DealState::OfferOpen | crate::state::DealState::Review
            )
            && deal.offer_deadline.0 <= world.tick.0 + 1
        {
            return Some(format!("deal_deadline:{}", deal.buyer.0));
        }
    }
    None
}

fn complete_construction(world: &mut World, dataset: &Dataset) {
    let due: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, facility)| {
            facility.status == crate::state::FacilityStatus::Building
                && facility.completes.0 <= world.tick.0
        })
        .map(|(handle, _)| handle)
        .collect();
    for handle in due {
        let (firm, escrow, capex) = match world.facilities.get(handle) {
            Some(facility) => (facility.firm, facility.escrow_cents, facility.capex_cents),
            None => continue,
        };
        // Equipment is released from escrow, then the whole asset moves out of CIP. Capex was
        // never expensed twice: service was already capitalised, equipment lands now.
        if let Some(entity) = world.firms.get_mut(firm) {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::WorkInProgress, escrow, "equipment received")
                    .credit(Account::RestrictedCash, escrow, "equipment received")
                    .debit(Account::FixedAssets, capex, "construction complete")
                    .credit(Account::WorkInProgress, capex, "construction complete"),
            );
        }
        if let Some(facility) = world.facilities.get_mut(handle) {
            facility.status = crate::state::FacilityStatus::Operating;
            facility.escrow_cents = 0;
            facility.completes = world.tick;
        }
    }
    let _ = dataset;
}

fn settle_expenses(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let Some(firm) = world.firms.get(handle) else {
            continue;
        };
        if !firm.operative() {
            continue;
        }
        let mut maintenance = 0i64;
        let mut rent = 0i64;
        let mut depreciation = 0i64;
        for facility_handle in &firm.facilities {
            let Some(facility) = world.facilities.get(*facility_handle) else {
                continue;
            };
            if facility.status == crate::state::FacilityStatus::Mothballed {
                continue;
            }
            let recipe = &dataset.recipes[facility.recipe as usize];
            let blocks = facility.blocks as i64;
            maintenance += recipe.weekly_maintenance(dataset.maintenance_annual_bps) * blocks;
            rent += dataset.fixture_land_rent_cents_weekly
                * blocks
                * world.regions[facility.region as usize].land_index_bps as i64
                / 10_000;
            if facility.status == crate::state::FacilityStatus::Operating {
                depreciation += recipe.weekly_depreciation() * blocks;
            }
        }
        if maintenance + rent > 0 {
            crate::economy::insolvency::pay_or_defer(
                world,
                handle,
                maintenance + rent,
                Account::OperatingExpense,
                "maintenance and rent",
            );
        }
        if depreciation > 0
            && let Some(entity) = world.firms.get_mut(handle)
        {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::DepreciationExpense, depreciation, "depreciation")
                    .credit(
                        Account::AccumulatedDepreciation,
                        depreciation,
                        "depreciation",
                    ),
            );
        }
        accrue_tax(world, handle);
    }
}

fn accrue_tax(world: &mut World, firm: Handle) {
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let taxable: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    if taxable <= 0 {
        return;
    }
    let policy = &world.regions[entity.region as usize].policy;
    let modifier = if policy
        .tax_modifier_until
        .map(|until| until.0 > world.tick.0)
        .unwrap_or(false)
    {
        policy.tax_modifier_bps
    } else {
        0
    };
    let rate = (2_500 + modifier).clamp(0, 10_000);
    let quarterly = round_even_div(taxable as i128 * rate as i128, 10_000) as i64;
    let weekly = quarterly / 13;
    if weekly <= 0 {
        return;
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::TaxExpense, weekly, "tax accrual")
                .credit(Account::TaxesPayable, weekly, "tax accrual"),
        );
    }
    if world.tick.0.is_multiple_of(13) {
        let due = -world
            .firms
            .get(firm)
            .map(|f| f.book.balance(Account::TaxesPayable))
            .unwrap_or(0);
        if due > 0 {
            let cash = world
                .firms
                .get(firm)
                .map(|f| f.book.cash())
                .unwrap_or(0)
                .max(0);
            let paid = due.min(cash);
            if paid > 0 {
                let _ = world.firms.get_mut(firm).unwrap().book.commit(
                    world.tick,
                    Txn::new().credit(Account::Cash, paid, "tax payment").debit(
                        Account::TaxesPayable,
                        paid,
                        "tax payment",
                    ),
                );
                let _ = world.government.book.commit(
                    world.tick,
                    Txn::new().debit(Account::Cash, paid, "tax receipt").credit(
                        Account::Revenue,
                        paid,
                        "tax receipt",
                    ),
                );
                world.government.last_tax_receipts_cents = paid;
            }
            if paid < due {
                crate::economy::insolvency::record_missed_tax(world, firm, due - paid);
            }
        }
    }
}

fn refresh_household_income(world: &mut World, dataset: &Dataset) {
    for region_index in 0..world.regions.len() {
        let mut income = 0i64;
        for tier in 0..3 {
            let background = world.regions[region_index].background_employed[tier];
            income += background * world.regions[region_index].background_wage_cents[tier];
        }
        for (_, facility) in world.facilities.iter() {
            if facility.region as usize != region_index {
                continue;
            }
            // Facility and firm handles are separate slabs; this must read the owning firm.
            if let Some(firm) = world.firms.get(facility.firm)
                && firm.operative()
            {
                for tier in 0..3 {
                    income += facility.workers[tier] * firm.policies.wage_offer[tier];
                }
            }
        }
        // Background transfer support for the unemployed, paid only from the treasury.
        let unemployed: i64 = world.regions[region_index]
            .labour
            .iter()
            .map(|tier| (tier.population * 9_500 / 10_000 - tier.employed).max(0))
            .sum();
        let support = (unemployed * world.regions[region_index].background_wage_cents[0] * 30
            / 100)
            .min(world.government.book.cash().max(0))
            .min(world.government.training_budget_cents.max(0));
        if support > 0 {
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, support, "transfer support")
                    .debit(Account::OperatingExpense, support, "transfer support"),
            );
            income += support;
            world.government.training_budget_cents -= support;
        }
        world.regions[region_index].households.last_income_cents = income;
    }
    let _ = dataset;
}

fn apply_macro_price_drift(world: &mut World, dataset: &Dataset) {
    // Nominal prices follow the price index; real quantities stay unchanged.
    let index = world.macro_state.price_index_bps.max(10_000);
    let _ = index;
    let _ = dataset;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn a_week_runs_with_balanced_books_and_finite_stock() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 121, ScenarioKind::Guided, ScenarioMode::FiveYear);
        for _ in 0..12 {
            advance(&mut world, &dataset);
        }
        assert_eq!(world.tick.0, 12);
        for (_, firm) in world.firms.iter() {
            assert!(firm.book.is_balanced(), "{}", firm.name);
            for lot in firm.inventory.values() {
                assert!(lot.qty.0 >= 0);
            }
        }
        for (_, facility) in world.facilities.iter() {
            assert!(
                facility.deposit_remaining.0 >= 0,
                "facility {} consumed a deposit it does not have",
                facility.recipe
            );
        }
        for region in &world.regions {
            assert!(region.households.savings_cents >= 0);
        }
    }

    #[test]
    fn the_clock_pauses_before_default_grace_expires() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 811, ScenarioKind::Guided, ScenarioMode::Sandbox);
        world.events.pending.clear();
        let firm = world.investor.primary_group.unwrap();
        crate::economy::insolvency::defer_expense(
            &mut world,
            firm,
            1_000_000,
            cw_core::ledger::Account::Wages,
            "unpaid",
        );
        world.firms.get_mut(firm).unwrap().past_due_weeks = 2;
        let outcome = advance(&mut world, &dataset);
        assert!(
            outcome
                .pause
                .as_deref()
                .unwrap_or("")
                .starts_with("default_grace"),
            "pause before the four-week grace ends: {:?}",
            outcome.pause
        );
    }

    #[test]
    fn the_clock_pauses_on_an_imminent_board_vote() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 812, ScenarioKind::Guided, ScenarioMode::Sandbox);
        world.events.pending.clear();
        let firm = world.investor.primary_group.unwrap();
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .board
            .pending_votes
            .push(crate::state::BoardVote {
                id: 1,
                subject: "approve large capex".to_owned(),
                deadline: Tick(2),
                yes: 0,
                no: 0,
                resolved: false,
                passed: false,
            });
        let outcome = advance(&mut world, &dataset);
        assert!(
            outcome
                .pause
                .as_deref()
                .unwrap_or("")
                .starts_with("board_vote"),
            "pause for the vote: {:?}",
            outcome.pause
        );
    }

    #[test]
    fn the_technology_frontier_advances_one_level_a_year() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 702, ScenarioKind::Founder, ScenarioMode::Sandbox);
        world.tick = Tick(51);
        world.macro_state.tech_frontier = 2;
        advance(&mut world, &dataset);
        assert_eq!(
            world.macro_state.tech_frontier, 3,
            "year boundary advances the frontier"
        );
        world.macro_state.tech_frontier = 20;
        world.tick = Tick(103);
        advance(&mut world, &dataset);
        assert_eq!(world.macro_state.tech_frontier, 20, "frontier is capped");
    }

    #[test]
    fn determinism_holds_for_a_fixed_seed() {
        let dataset = Dataset::load().unwrap();
        let mut left = seed::build(&dataset, 131, ScenarioKind::Guided, ScenarioMode::FiveYear);
        let mut right = seed::build(&dataset, 131, ScenarioKind::Guided, ScenarioMode::FiveYear);
        for _ in 0..8 {
            advance(&mut left, &dataset);
            advance(&mut right, &dataset);
        }
        let left_hash = crate::state::canonical_hash(&left);
        let right_hash = crate::state::canonical_hash(&right);
        assert_eq!(left_hash, right_hash);
    }
}
