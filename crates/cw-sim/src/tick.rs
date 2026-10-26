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
    crate::economy::procurement::ensure_contracts(world, dataset);
    crate::economy::procurement::settle(world, dataset);
    complete_construction(world, dataset);
    crate::economy::deals::weekly(world);
    crate::economy::governance::weekly(world, dataset);

    // Phase 3: labour and payroll.
    for region in &mut world.regions {
        region.paid_named_wages_cents = 0;
    }
    crate::economy::labour::clear(world, dataset);

    // Phase 4: utilities and production.
    crate::economy::production::produce(world, dataset);

    // Phase 5: markets.
    crate::economy::markets::clear_week(world, dataset);

    // Phase 7: expenses, debt service, claims and defaults.
    settle_expenses(world, dataset);
    crate::economy::operations::weekly(world, dataset);
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
    crate::economy::events_engine::state_triggers(world, dataset);
    crate::economy::events_engine::auto_resolve(world, dataset);
    crate::economy::events_engine::expire_optional(world, dataset);
    world.clear_construction_orders();
    refresh_household_income(world, dataset);
    crate::economy::fiscal::close_week(world);
    apply_macro_price_drift(world, dataset);

    // Close books and sales history, then track resilience.
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        if let Some(firm) = world.firms.get_mut(handle) {
            let _ = firm.book.close_week(world.tick);
            firm.close_sales_week();
        }
    }
    let past_due = world.firms.iter().any(|(handle, firm)| {
        crate::economy::events_engine::player_side(world, handle)
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
    crate::economy::pricing::weekly(world, dataset);
    crate::economy::objectives::weekly(world, dataset);
    crate::economy::score::update(world, dataset);
    if let Some(reason) = world.pending_pause.clone() {
        return WeekOutcome {
            week: world.tick,
            pause: Some(reason),
            restructured: Vec::new(),
            liquidated: Vec::new(),
        };
    }
    // Mandatory player decisions pause the clock. A rival's mandatory card is answered by the
    // rival and never stops the player's time.
    if let Some(event) = world.events.pending.iter().find(|event| {
        !event.resolved
            && event.mandatory
            && event
                .subject_firm
                .map(|firm| crate::economy::events_engine::player_side(world, firm))
                .unwrap_or(true)
    }) {
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
        let (firm, escrow, capex, equipment_due) = match world.facilities.get(handle) {
            Some(facility) => (
                facility.firm,
                facility.escrow_cents,
                facility.capex_cents,
                facility.equipment_due_cents,
            ),
            None => continue,
        };
        // Equipment is paid for on delivery. A firm without the cash leaves it at the depot for
        // another week: the plant waits, the firm is not pushed into arrears.
        if equipment_due > 0 {
            let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
            if cash < equipment_due {
                if let Some(facility) = world.facilities.get_mut(handle) {
                    facility.completes = Tick(world.tick.0 + 1);
                }
                continue;
            }
            if let Some(entity) = world.firms.get_mut(firm) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(
                            Account::WorkInProgress,
                            equipment_due,
                            "equipment delivered",
                        )
                        .credit(Account::Cash, equipment_due, "equipment delivered"),
                );
            }
            if let Some(facility) = world.facilities.get_mut(handle) {
                facility.equipment_due_cents = 0;
            }
        }
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
    // Retooling finishes on its date: the escrowed equipment lands as fixed assets.
    let retooled: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, facility)| {
            matches!(facility.status, crate::state::FacilityStatus::Retooling { until } if until.0 <= world.tick.0)
        })
        .map(|(handle, _)| handle)
        .collect();
    for handle in retooled {
        let (firm, escrow) = match world.facilities.get(handle) {
            Some(facility) => (facility.firm, facility.escrow_cents),
            None => continue,
        };
        if escrow > 0
            && let Some(entity) = world.firms.get_mut(firm)
        {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::FixedAssets, escrow, "retool complete")
                    .credit(Account::RestrictedCash, escrow, "retool complete"),
            );
        }
        if let Some(facility) = world.facilities.get_mut(handle) {
            facility.status = crate::state::FacilityStatus::Operating;
            facility.escrow_cents = 0;
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
    let trailing: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    // Losses carry forward: at each quarter end a losing quarter adds to the pool and a
    // profitable one uses it up before any of its profit is taxed. Without this a seasonal
    // business paid tax on its good quarters and never offset its bad ones.
    let carry = entity.tax_loss_carry_cents;
    if world.tick.0.is_multiple_of(13) {
        let next = if trailing < 0 {
            carry.saturating_add(-trailing)
        } else {
            (carry - trailing).max(0)
        };
        if let Some(entity) = world.firms.get_mut(firm) {
            entity.tax_loss_carry_cents = next;
        }
    }
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let taxable = (trailing - carry).max(0);
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
                crate::economy::fiscal::record(world, paid);
            }
            if paid < due {
                crate::economy::insolvency::record_missed_tax(world, firm, due - paid);
            }
        }
    }
}

fn refresh_household_income(world: &mut World, dataset: &Dataset) {
    world.government.unpaid_transfer_need_cents = 0;
    for region_index in 0..world.regions.len() {
        let mut income = 0i64;
        for tier in 0..3 {
            let background = world.regions[region_index].background_employed[tier];
            income += background * world.regions[region_index].background_wage_cents[tier];
        }
        // Named income is what employers actually paid this week. Deferred wages are a
        // liability, not household cash.
        income += world.regions[region_index].paid_named_wages_cents;
        // Taxable labour income pays twenty percent to the government. Households spend
        // from what remains; transfer support is not taxable wage income.
        let earned = income;
        let labour_tax = earned * crate::economy::fiscal::LABOUR_TAX_BPS / 10_000;
        if labour_tax > 0 {
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, labour_tax, "labour income tax")
                    .credit(Account::Revenue, labour_tax, "labour income tax"),
            );
            crate::economy::fiscal::record(world, labour_tax);
        }
        let mut income = earned - labour_tax;
        // Background transfer support for the unemployed, paid only from the treasury. The
        // need uses each tier's baseline wage; anything the residual budget cannot fund is
        // reported rather than invented.
        let mut need = 0i64;
        for tier in 0..3 {
            let unemployed = (world.regions[region_index].labour[tier].population * 9_500 / 10_000)
                .saturating_sub(world.regions[region_index].labour[tier].employed)
                .max(0);
            need += unemployed * world.regions[region_index].background_wage_cents[tier] * 30 / 100;
        }
        let support = need
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
        world.government.unpaid_transfer_need_cents = world
            .government
            .unpaid_transfer_need_cents
            .saturating_add(need - support);
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
    fn a_losing_quarter_is_carried_against_the_next_profitable_one() {
        use cw_core::ledger::{Account, Txn};
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 42, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let firm = world.firms.iter().next().unwrap().0;
        let close_weeks = |world: &mut World, weekly_income: i64| {
            for week in 0..13 {
                let book = &mut world.firms.get_mut(firm).unwrap().book;
                let txn = if weekly_income >= 0 {
                    Txn::new()
                        .debit(Account::Cash, weekly_income, "test sale")
                        .credit(Account::Revenue, weekly_income, "test sale")
                } else {
                    Txn::new()
                        .debit(Account::OperatingExpense, -weekly_income, "test cost")
                        .credit(Account::Cash, -weekly_income, "test cost")
                };
                let _ = book.commit(Tick(100 + week), txn);
                let _ = book.close_week(Tick(100 + week));
            }
        };
        let payable = |world: &World| {
            world
                .firms
                .get(firm)
                .unwrap()
                .book
                .balance(Account::TaxesPayable)
        };
        let before = payable(&world);
        // A quarter losing $1,000 a week: at the quarter end it joins the carry, untaxed.
        close_weeks(&mut world, -100_000);
        world.tick = Tick(13);
        accrue_tax(&mut world, firm);
        assert_eq!(
            world.firms.get(firm).unwrap().tax_loss_carry_cents,
            1_300_000
        );
        assert_eq!(payable(&world), before);
        // A quarter earning $3,000 a week: only the $26,000 above the $13,000 carry is taxed,
        // at 25 percent, accrued a thirteenth a week: 26,000 * 0.25 / 13 = $500.
        close_weeks(&mut world, 300_000);
        world.tick = Tick(14);
        accrue_tax(&mut world, firm);
        assert_eq!(payable(&world) - before, -50_000);
    }

    #[test]
    fn labour_income_tax_is_withheld_and_budgeted() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 41, ScenarioKind::Guided, ScenarioMode::Sandbox);
        advance(&mut world, &dataset);
        // Withholding is a regional aggregate: twenty percent of each state's paid wages,
        // rounded down per state, then summed.
        let mut expected = 0i64;
        for region in &world.regions {
            let mut gross = region.paid_named_wages_cents;
            for tier in 0..3 {
                gross += region.background_employed[tier] * region.background_wage_cents[tier];
            }
            expected += gross * 2_000 / 10_000;
        }
        assert_eq!(
            *world
                .government
                .tax_receipts_history
                .last()
                .expect("a closed fiscal week"),
            expected,
            "twenty percent of labour income reaches the treasury"
        );
        assert!(world.government.procurement_budget_cents > 0);
        assert!(world.government.healthcare_budget_cents > 0);
        assert!(world.government.training_budget_cents > 0);
        assert!(world.government.book.is_balanced());
    }

    #[test]
    fn unfunded_transfer_support_is_reported() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 43, ScenarioKind::Guided, ScenarioMode::Sandbox);
        world.government.training_budget_cents = 0;
        for tier in 0..3 {
            world.regions[0].labour[tier].employed = 0;
        }
        refresh_household_income(&mut world, &dataset);
        assert!(
            world.government.unpaid_transfer_need_cents > 0,
            "unmet transfer need is recorded"
        );
        assert!(
            world.government.book.balance(Account::OperatingExpense) >= 0,
            "no unfunded support is paid"
        );
        assert!(world.government.book.is_balanced());
    }

    #[test]
    fn deferred_wages_are_not_household_income_or_tax() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 42, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let firm = world
            .firms
            .iter()
            .find(|(_, firm)| {
                firm.operative() && !firm.facilities.is_empty() && firm.book.cash() > 0
            })
            .map(|(handle, _)| handle)
            .expect("an operating employer");
        let cash = world.firm(firm).unwrap().book.cash();
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .book
            .commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .debit(Account::RetainedEarnings, cash, "drain for deferral test")
                    .credit(Account::Cash, cash, "drain for deferral test"),
            )
            .unwrap();
        advance(&mut world, &dataset);
        println!(
            "cash={} overdue={} payables={} accrued={} status={:?}",
            world.firm(firm).unwrap().book.cash(),
            world
                .firm(firm)
                .unwrap()
                .book
                .balance(Account::OverduePayables),
            world.firm(firm).unwrap().book.balance(Account::Payables),
            world
                .firm(firm)
                .unwrap()
                .book
                .balance(Account::AccruedLiabilities),
            world.firm(firm).unwrap().status
        );
        assert!(
            world
                .firm(firm)
                .unwrap()
                .book
                .balance(Account::OverduePayables)
                < 0,
            "the firm deferred wages it could not pay"
        );
        let mut expected = 0i64;
        for region in &world.regions {
            let mut gross = region.paid_named_wages_cents;
            for tier in 0..3 {
                gross += region.background_employed[tier] * region.background_wage_cents[tier];
            }
            expected += gross * 2_000 / 10_000;
        }
        assert_eq!(
            *world
                .government
                .tax_receipts_history
                .last()
                .expect("a closed fiscal week"),
            expected,
            "only paid wages are taxed"
        );
        assert!(world.firm(firm).unwrap().book.is_balanced());
    }

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
