//! Property insurance underwriting, reserves and claims.
//!
//! Annual quoted premium = limit * 200 bps * 1.25. Premiums are paid weekly with a carry.
//! An insurer accepts a policy only when capital covers ten percent of total coverage and
//! unrestricted cash covers two weeks of overhead plus five percent of coverage, and regional
//! concentration stays within forty percent unless it chooses specialist mode with double the
//! capital requirement. Idiosyncratic loss frequency uses the severity-correct hazard:
//! annual incident probability = round_even(200 / 19.5) = 1026 bps. Severity is 10/30/100 percent
//! of asset value with weights 70/25/5. One region catastrophe draw applies to every affected
//! policy at once.

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;
use cw_core::rng::{Rng, TAG_INSURANCE};

use crate::state::{InsurancePolicy, PolicyStatus, World};

pub const BASE_RISK_BPS: i32 = 200;
pub const ANNUAL_INCIDENT_BPS: i32 = 1_026;
const SEVERITY_WEIGHTS: [i64; 3] = [70, 25, 5];
const SEVERITY_BPS: [i64; 3] = [1_000, 3_000, 10_000];

pub fn weekly_premium(limit_cents: i64) -> i64 {
    let annual = round_even_div(limit_cents as i128 * BASE_RISK_BPS as i128, 10_000) as i64;
    let annual = annual * 125 / 100;
    round_even_div(annual as i128, 52) as i64
}

/// Quote a policy, checking the insurer's capital, liquidity and concentration limits.
pub fn quote(world: &World, insurer: Handle, region: u8, limit_cents: i64) -> DomainResult<i64> {
    let Some(firm) = world.firms.get(insurer) else {
        return Err(DomainError::InvalidEntity {
            handle: insurer.0,
            kind: EntityKind::Insurer,
        });
    };
    let Some(model) = firm.insurer.as_ref() else {
        return Err(DomainError::InvalidEntity {
            handle: insurer.0,
            kind: EntityKind::Insurer,
        });
    };
    let coverage: i64 = world
        .insurance
        .iter()
        .filter(|(_, policy)| policy.insurer == insurer && policy.status == PolicyStatus::Active)
        .map(|(_, policy)| policy.limit_cents)
        .sum();
    let capital_need = (coverage + limit_cents) * 10 / 100;
    let capital_need = if model.specialist_concentration {
        capital_need * 2
    } else {
        capital_need
    };
    if firm.book.total_equity() < capital_need {
        return Err(DomainError::SolvencyRestriction {
            entity: insurer.0,
            reason: cw_core::error::SolvencyReason::CapitalTooLow,
        });
    }
    let overhead = 2 * crate::economy::insolvency::mandatory_weekly_outflow(world, insurer);
    let cash_need = overhead + (coverage + limit_cents) * 5 / 100;
    if firm.book.cash() < cash_need {
        return Err(DomainError::SolvencyRestriction {
            entity: insurer.0,
            reason: cw_core::error::SolvencyReason::LiquidityTooLow,
        });
    }
    let region_coverage: i64 = world
        .insurance
        .iter()
        .filter(|(_, policy)| {
            policy.insurer == insurer
                && policy.region == region
                && policy.status == PolicyStatus::Active
        })
        .map(|(_, policy)| policy.limit_cents)
        .sum();
    if !model.specialist_concentration {
        // The first policy necessarily concentrates the book in one region. Allow it to open
        // the business, then require diversification: later policies must keep any one region
        // at or below forty percent of total coverage.
        let total = coverage + limit_cents;
        if coverage > 0 && total > 0 && (region_coverage + limit_cents) * 100 > total * 40 {
            return Err(DomainError::SolvencyRestriction {
                entity: insurer.0,
                reason: cw_core::error::SolvencyReason::CoverageTooLow,
            });
        }
    }
    Ok(weekly_premium(limit_cents))
}

/// Bind a policy for a declared facility.
pub fn bind(
    world: &mut World,
    insurer: Handle,
    insured: Handle,
    facility: Handle,
    limit_cents: i64,
    deductible_cents: i64,
) -> DomainResult<Handle> {
    let region = world
        .facilities
        .get(facility)
        .map(|facility| facility.region)
        .ok_or(DomainError::InvalidEntity {
            handle: facility.0,
            kind: EntityKind::Facility,
        })?;
    let premium = quote(world, insurer, region, limit_cents)?;
    let specialist = world
        .firms
        .get(insurer)
        .and_then(|f| f.insurer.as_ref())
        .map(|m| m.specialist_concentration)
        .unwrap_or(false);
    let policy = InsurancePolicy {
        insurer,
        insured,
        facility,
        region,
        limit_cents,
        deductible_cents,
        weekly_premium_cents: premium,
        start: world.tick,
        term_weeks: 52,
        earned_premium_cents: 0,
        claim_reserve_cents: 0,
        claim_payable_week: 0,
        status: PolicyStatus::Active,
        specialist,
    };
    let handle = world.insurance.insert(policy);
    if let Some(firm) = world.firms.get_mut(insured) {
        firm.insurance.push(handle);
    }
    Ok(handle)
}

/// Pay recognised claims whose payable week has arrived, from actual insurer cash.
pub fn settle_claims(world: &mut World) {
    let handles: Vec<Handle> = world.insurance.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let Some(policy) = world.insurance.get(handle).cloned() else {
            continue;
        };
        if policy.status != PolicyStatus::Active
            || policy.claim_reserve_cents <= 0
            || policy.claim_payable_week > world.tick.0
        {
            continue;
        }
        let cash = world
            .firms
            .get(policy.insurer)
            .map(|firm| firm.book.cash())
            .unwrap_or(0)
            .max(0);
        let paid = policy.claim_reserve_cents.min(cash);
        if paid <= 0 {
            continue;
        }
        let _ = world.firms.get_mut(policy.insurer).unwrap().book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, paid, "claim payment")
                .debit(Account::ClaimLiabilities, paid, "claim payment"),
        );
        let _ = world.firms.get_mut(policy.insured).unwrap().book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, paid, "claim received")
                .credit(Account::OtherIncome, paid, "claim received"),
        );
        if let Some(stored) = world.insurance.get_mut(handle) {
            stored.claim_reserve_cents = stored.claim_reserve_cents.saturating_sub(paid);
        }
    }
}

/// Weekly premiums, earned premium recognition, idiosyncratic losses and claims payment.
pub fn weekly(world: &mut World) {
    settle_claims(world);
    let handles: Vec<Handle> = world.insurance.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let Some(policy) = world.insurance.get(handle).cloned() else {
            continue;
        };
        if policy.status != PolicyStatus::Active {
            continue;
        }
        if world.tick.0 >= policy.start.0 + policy.term_weeks {
            if let Some(stored) = world.insurance.get_mut(handle) {
                stored.status = PolicyStatus::Expired;
            }
            continue;
        }
        // Premium: paid by the insured, earned by the insurer over coverage.
        let premium = policy.weekly_premium_cents;
        let insured_cash = world
            .firms
            .get(policy.insured)
            .map(|f| f.book.cash())
            .unwrap_or(0)
            .max(0);
        let paid = premium.min(insured_cash);
        if paid > 0 {
            let _ = world.firms.get_mut(policy.insured).unwrap().book.commit(
                world.tick,
                Txn::new().credit(Account::Cash, paid, "premium").debit(
                    Account::OperatingExpense,
                    paid,
                    "premium",
                ),
            );
            let _ = world.firms.get_mut(policy.insurer).unwrap().book.commit(
                world.tick,
                Txn::new().debit(Account::Cash, paid, "premium").credit(
                    Account::Revenue,
                    paid,
                    "premium",
                ),
            );
        } else {
            crate::economy::insolvency::defer_expense(
                world,
                policy.insured,
                premium,
                Account::OperatingExpense,
                "unpaid premium",
            );
        }
        if let Some(stored) = world.insurance.get_mut(handle) {
            stored.earned_premium_cents += paid;
        }

        // Idiosyncratic loss: weekly threshold = annual incident probability / 52, using a
        // per-policy accumulator-safe stream.
        let mut rng = Rng::entity_stream(world.seed, TAG_INSURANCE, handle);
        let chance = ANNUAL_INCIDENT_BPS as u64;
        let roll = rng.uniform_below(10_000 * 52);
        if roll < chance {
            let facility_value = world
                .facilities
                .get(policy.facility)
                .map(|facility| facility.capex_cents - facility.accumulated_depreciation)
                .unwrap_or(0)
                .max(0);
            let severity_index = rng.weighted_index(&SEVERITY_WEIGHTS);
            let severity_bps = SEVERITY_BPS[severity_index];
            let loss = facility_value * severity_bps / 10_000;
            apply_loss(world, handle, &policy, loss);
        }
    }
}

fn apply_loss(world: &mut World, handle: Handle, policy: &InsurancePolicy, loss_cents: i64) {
    if loss_cents <= 0 {
        return;
    }
    // Physical damage happens once: reduce the facility's carrying value and capacity.
    if let Some(facility) = world.facilities.get_mut(policy.facility) {
        facility.accumulated_depreciation += loss_cents;
        facility.target_bps = (facility.target_bps * 70 / 100).max(2_000);
    }
    let claim = (loss_cents - policy.deductible_cents).clamp(0, policy.limit_cents);
    if let Some(stored) = world.insurance.get_mut(handle) {
        stored.claim_reserve_cents += claim;
        stored.claim_payable_week = world.tick.0 + 1;
    }
    if claim > 0 {
        // Recognise the expense and reserve now; the payout settles next week from actual cash.
        let _ = world.firms.get_mut(policy.insurer).unwrap().book.commit(
            world.tick,
            Txn::new()
                .debit(Account::OperatingExpense, claim, "claim reserve")
                .credit(Account::ClaimLiabilities, claim, "claim reserve"),
        );
    }
}

/// One correlated catastrophe per region: 30 percent of insured asset values, one draw.
pub fn catastrophe(world: &mut World, region: u8) {
    let affected: Vec<Handle> = world
        .insurance
        .iter()
        .filter(|(_, policy)| policy.region == region && policy.status == PolicyStatus::Active)
        .map(|(handle, _)| handle)
        .collect();
    if affected.is_empty() {
        return;
    }
    let mut rng = Rng::stream(world.seed ^ region as u64, cw_core::rng::TAG_EVENTS);
    let _ = rng.uniform_below(100);
    for handle in affected {
        let Some(policy) = world.insurance.get(handle).cloned() else {
            continue;
        };
        let facility_value = world
            .facilities
            .get(policy.facility)
            .map(|facility| (facility.capex_cents - facility.accumulated_depreciation).max(0))
            .unwrap_or(0);
        apply_loss(world, handle, &policy, facility_value * 30 / 100);
    }
    if let Some(region_state) = world.regions.get_mut(region as usize) {
        region_state.catastrophe_until = Some(Tick(world.tick.0 + 26));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cw_data::Dataset;

    #[test]
    fn annual_premium_uses_the_prescribed_formula() {
        // $1,000,000 limit: 2% expected loss = $20,000, times 1.25 = $25,000 a year.
        assert_eq!(weekly_premium(100_000_000), 48_077);
        let annual = weekly_premium(100_000_000) * 52;
        assert!((2_499_000..=2_501_000).contains(&annual));
    }

    #[test]
    fn a_claim_is_reserved_now_and_paid_next_week() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            801,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let insurer = world
            .firms
            .iter()
            .find(|(_, firm)| firm.insurer.is_some())
            .map(|(handle, _)| handle)
            .unwrap();
        let insured = world
            .firms
            .iter()
            .find(|(_, firm)| firm.insurer.is_none() && !firm.facilities.is_empty())
            .map(|(handle, _)| handle)
            .unwrap();
        let facility = world.firm(insured).unwrap().facilities[0];
        let limit = world.facilities.get(facility).unwrap().capex_cents / 2;
        let policy = bind(&mut world, insurer, insured, facility, limit, 0).unwrap();
        // Isolate the claim from the premium cash flow.
        world
            .insurance
            .get_mut(policy)
            .unwrap()
            .weekly_premium_cents = 0;
        let cash_before = world.firm(insured).unwrap().book.cash();
        let region = world.facilities.get(facility).unwrap().region;
        catastrophe(&mut world, region);
        let reserved = world.insurance.get(policy).unwrap().claim_reserve_cents;
        assert!(reserved > 0, "the incident is reserved");
        assert_eq!(
            world.firm(insured).unwrap().book.cash(),
            cash_before,
            "no payout yet"
        );
        world.tick = Tick(2);
        weekly(&mut world);
        assert!(
            world.firm(insured).unwrap().book.cash() > cash_before,
            "paid next week"
        );
        assert!(world.insurance.get(policy).unwrap().claim_reserve_cents < reserved);
        assert!(world.firm(insurer).unwrap().book.is_balanced());
    }

    #[test]
    fn severity_correct_hazard_is_about_the_prescribed_value() {
        // 200 bps loss ratio / 0.195 expected severity = 1026 bps incident probability.
        let expected_severity = SEVERITY_WEIGHTS
            .iter()
            .zip(SEVERITY_BPS.iter())
            .map(|(weight, severity)| weight * severity)
            .sum::<i64>()
            / 100;
        assert_eq!(expected_severity, 1_950);
        assert_eq!(ANNUAL_INCIDENT_BPS, 1_026);
        let expected_loss = ANNUAL_INCIDENT_BPS as i64 * expected_severity / 10_000;
        assert!((195..=205).contains(&expected_loss));
    }
}
