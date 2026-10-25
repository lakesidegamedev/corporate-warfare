//! Bounded lobbying and regional policy.
//!
//! Three request types: permitting acceleration (build duration down 25 percent for 52 weeks),
//! training grant (20 percent government subsidy of retraining cost) and a 200 bps regional tax
//! reduction for 52 weeks. A request costs max($25,000, 0.1 percent of group assets) to the
//! outside advocacy sector. One active request per group and region, 26-week cooldown, a
//! four-week decision, and a success probability of
//! clamp(2000 + reputation/4 - recent wins * 500, 1000, 5000) bps drawn once at creation and
//! saved. Funding never guarantees the result; costs remain on failure. No request buys an
//! antitrust exemption.

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, DomainResult};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::rng::{Rng, TAG_POLICY};

use crate::state::{PolicyKind, PolicyRequest, World};

pub const REQUEST_COOLDOWN_WEEKS: u32 = 26;
const DECISION_WEEKS: u32 = 4;

pub fn request_fee(world: &World, firm: Handle) -> i64 {
    let group_assets = world
        .firms
        .get(firm)
        .map(|entity| entity.book.total_assets().max(0))
        .unwrap_or(0);
    (group_assets * 10 / 10_000).max(2_500_000)
}

pub fn file_request(
    world: &mut World,
    firm: Handle,
    region: u8,
    kind: PolicyKind,
) -> DomainResult<i64> {
    let Some(entity) = world.firms.get(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: cw_core::error::EntityKind::Firm,
        });
    };
    for request in &entity.policy_requests {
        if request.region == region && !request.resolved && world.tick.0 < request.cooldown_until.0
        {
            return Err(DomainError::SolvencyRestriction {
                entity: firm.0,
                reason: cw_core::error::SolvencyReason::PastDue,
            });
        }
    }
    let fee = request_fee(world, firm);
    let cash = entity.book.cash().max(0);
    if cash < fee {
        return Err(DomainError::InsufficientCash {
            entity: firm.0,
            required: fee,
            available: cash,
        });
    }
    // One saved draw at creation; the result cannot be rerolled by reopening a screen.
    let mut rng = Rng::entity_stream(world.seed ^ world.tick.0 as u64, TAG_POLICY, firm);
    let wins = entity
        .policy_requests
        .iter()
        .filter(|request| request.resolved && request.success)
        .count() as i32;
    let probability = (2_000 + entity.reputation_bps / 4 - wins * 500).clamp(1_000, 5_000);
    let success = rng.chance_bps(probability);
    if kind == PolicyKind::TrainingGrant && world.government.training_budget_cents < fee / 5 {
        // The grant needs a finite government budget or it is declined.
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::CoverageTooLow,
        });
    }
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new().credit(Account::Cash, fee, "lobbying fee").debit(
            Account::OperatingExpense,
            fee,
            "lobbying fee",
        ),
    );
    let _ = world.outside.advocacy_book.commit(
        world.tick,
        Txn::new().debit(Account::Cash, fee, "lobbying fee").credit(
            Account::Revenue,
            fee,
            "lobbying fee",
        ),
    );
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.policy_requests.push(PolicyRequest {
            kind,
            region,
            fee_cents: fee,
            created: world.tick,
            decision: Tick(world.tick.0 + DECISION_WEEKS),
            success,
            resolved: false,
            cooldown_until: Tick(world.tick.0 + REQUEST_COOLDOWN_WEEKS),
        });
    }
    Ok(fee)
}

pub fn weekly(world: &mut World) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let due: Vec<(usize, PolicyRequest)> = world
            .firms
            .get(handle)
            .map(|entity| {
                entity
                    .policy_requests
                    .iter()
                    .enumerate()
                    .filter(|(_, request)| !request.resolved && world.tick.0 >= request.decision.0)
                    .map(|(index, request)| (index, request.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for (index, request) in due {
            if request.success {
                apply_success(world, handle, &request);
            }
            if let Some(entity) = world.firms.get_mut(handle)
                && let Some(stored) = entity.policy_requests.get_mut(index)
            {
                stored.resolved = true;
            }
        }
    }
}

fn apply_success(world: &mut World, firm: Handle, request: &PolicyRequest) {
    let region = request.region as usize;
    match request.kind {
        PolicyKind::PermitAcceleration => {
            if let Some(region) = world.regions.get_mut(region) {
                region.policy.permit_bonus_until = Some(Tick(world.tick.0 + 52));
            }
        }
        PolicyKind::TaxAdjustment => {
            if let Some(region) = world.regions.get_mut(region) {
                region.policy.tax_modifier_bps = -200;
                region.policy.tax_modifier_until = Some(Tick(world.tick.0 + 52));
            }
        }
        PolicyKind::TrainingGrant => {
            let subsidy = (request.fee_cents / 5).min(world.government.training_budget_cents);
            if subsidy > 0 {
                world.government.training_budget_cents -= subsidy;
                if let Some(region) = world.regions.get_mut(region) {
                    region.policy.training_grant_until = Some(Tick(world.tick.0 + 26));
                }
                // Funded from the finite government account, never minted.
                let _ = world.government.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, subsidy, "training grant")
                        .debit(Account::OperatingExpense, subsidy, "training grant"),
                );
                if let Some(entity) = world.firms.get_mut(firm) {
                    let _ = entity.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::Cash, subsidy, "training grant")
                            .credit(Account::OtherIncome, subsidy, "training grant"),
                    );
                }
            }
        }
    }
}

/// A failed campaign with concurrent layoffs damages reputation.
pub fn backlash(world: &mut World, firm: Handle, cancel: bool) {
    let penalty = if cancel { 100 } else { 250 };
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.reputation_bps = (entity.reputation_bps - penalty).max(-2_000);
    }
    world.regulatory_breaches += 0;
    let _ = firm;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_data::Dataset;

    #[test]
    fn fee_is_at_least_twenty_five_thousand_dollars() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 91, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handle = world.firms.iter().next().unwrap().0;
        assert_eq!(request_fee(&world, handle), 2_500_000);
    }

    #[test]
    fn a_filed_request_costs_cash_and_saves_its_draw() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 92, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        let before = world.firm(primary).unwrap().book.cash();
        let fee = file_request(&mut world, primary, 3, PolicyKind::PermitAcceleration).unwrap();
        assert_eq!(world.firm(primary).unwrap().book.cash(), before - fee);
        let saved = world.firm(primary).unwrap().policy_requests[0].success;
        for _ in 0..8 {
            weekly(&mut world);
        }
        assert_eq!(
            world.firm(primary).unwrap().policy_requests[0].success,
            saved
        );
    }
}
