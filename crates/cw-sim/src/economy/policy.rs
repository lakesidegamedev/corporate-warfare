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
/// Wins inside this window lower the next request's odds by 500 bps each.
const RECENT_WIN_WEEKS: u32 = 104;
/// Layoffs are judged over the four weeks up to a decision.
const LAYOFF_WINDOW_WEEKS: u32 = 4;
/// A mass layoff releases at least a tenth of the group's workforce inside the window.
const MASS_LAYOFF_BPS: i64 = 1_000;
const BACKLASH_BPS: i32 = 250;

/// Group assets without double counting: each member's assets less its holdings in other
/// companies, since those holdings are the other members' assets again.
fn group_assets(world: &World, firm: Handle) -> i64 {
    crate::economy::deals::competition_group(world, firm)
        .into_iter()
        .filter_map(|member| world.firm(member))
        .map(|entity| {
            (entity.book.total_assets() - entity.book.balance(Account::Investments)).max(0)
        })
        .sum()
}

pub fn request_fee(world: &World, firm: Handle) -> i64 {
    (group_assets(world, firm) * 10 / 10_000).max(2_500_000)
}

/// Success odds in bps: clamp(2000 + reputation/4 - recent group wins * 500, 1000, 5000).
pub fn success_bps(world: &World, firm: Handle) -> i32 {
    let reputation = world.firm(firm).map(|f| f.reputation_bps).unwrap_or(0);
    let since = world.tick.0.saturating_sub(RECENT_WIN_WEEKS);
    let wins = crate::economy::deals::competition_group(world, firm)
        .into_iter()
        .filter_map(|member| world.firm(member))
        .flat_map(|entity| entity.policy_requests.iter())
        .filter(|request| request.resolved && request.success && request.created.0 >= since)
        .count() as i32;
    (2_000 + reputation / 4 - wins * 500).clamp(1_000, 5_000)
}

/// True while any company in the group has a request open or cooling down in the region.
pub fn cooling_down(world: &World, firm: Handle, region: u8) -> bool {
    crate::economy::deals::competition_group(world, firm)
        .into_iter()
        .filter_map(|member| world.firm(member))
        .flat_map(|entity| entity.policy_requests.iter())
        .any(|request| request.region == region && world.tick.0 < request.cooldown_until.0)
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
    if region as usize >= world.regions.len() {
        return Err(DomainError::InvalidEntity {
            handle: region as u64,
            kind: cw_core::error::EntityKind::Region,
        });
    }
    // One request per group and region, including the 26-week cooldown after a decision. The
    // group is the whole ultimate-controller group, so a sibling company cannot skip it.
    if cooling_down(world, firm, region) {
        return Err(DomainError::WrongPhase {
            expected: "policy_cooldown_elapsed",
            actual: "policy_cooldown",
        });
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
    if kind == PolicyKind::TrainingGrant && world.government.training_budget_cents <= 0 {
        // The grant needs a finite government budget or it is declined before any fee.
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::CoverageTooLow,
        });
    }
    // One saved draw at creation; the result cannot be rerolled by reopening a screen.
    let probability = success_bps(world, firm);
    let mut rng = Rng::entity_stream(world.seed ^ world.tick.0 as u64, TAG_POLICY, firm);
    let success = rng.chance_bps(probability);
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
    let horizon = world.tick.0.saturating_sub(LAYOFF_WINDOW_WEEKS);
    for handle in &handles {
        if let Some(entity) = world.firms.get_mut(*handle) {
            entity.layoffs.retain(|(week, _)| *week > horizon);
        }
    }
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
            // Opens the regional window in which retraining is twenty percent subsidised from
            // the finite government training budget (labour.rs). No cash is paid here.
            if let Some(region) = world.regions.get_mut(region) {
                region.policy.training_grant_until = Some(Tick(world.tick.0 + 26));
            }
        }
    }
    let _ = firm;
}

/// Workers the group released in the layoff window, against its current workforce.
pub fn mass_layoffs(world: &World, firm: Handle) -> bool {
    let group = crate::economy::deals::competition_group(world, firm);
    let released: i64 = group
        .iter()
        .filter_map(|member| world.firm(*member))
        .flat_map(|entity| entity.layoffs.iter())
        .map(|(_, count)| *count)
        .sum();
    let employed: i64 = group
        .iter()
        .filter_map(|member| world.firm(*member))
        .map(|entity| entity.total_employees(&world.facilities))
        .sum();
    released > 0 && released * 10_000 >= (employed + released) * MASS_LAYOFF_BPS
}

/// The lobby backlash card's outcome: disclosing a campaign run during mass layoffs costs 250
/// bps reputation, cancelling it costs 100 bps and forfeits the fee. Recorded publicly in the
/// news feed. The card is the only path, so the penalty is never applied twice.
pub fn backlash(world: &mut World, firm: Handle, cancel: bool) {
    let penalty = if cancel { 100 } else { BACKLASH_BPS };
    let Some(entity) = world.firms.get_mut(firm) else {
        return;
    };
    entity.reputation_bps = (entity.reputation_bps - penalty).max(-2_000);
    let subject = entity.name.clone();
    world.news.push(crate::state::NewsItem {
        week: world.tick,
        key: "news.lobbying_backlash".to_owned(),
        subject,
        amount_cents: penalty as i64,
        quantity: Default::default(),
    });
}

/// Cancel the firm's open campaigns: they resolve unsuccessful and the fee is not refunded.
pub fn cancel_open(world: &mut World, firm: Handle) {
    if let Some(entity) = world.firms.get_mut(firm) {
        for request in entity.policy_requests.iter_mut().filter(|r| !r.resolved) {
            request.resolved = true;
            request.success = false;
        }
    }
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
    fn the_cooldown_survives_resolution_and_spans_the_group() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            733,
            crate::state::ScenarioKind::Guided,
            crate::state::ScenarioMode::Sandbox,
        );
        let firm = world.investor.primary_group.unwrap();
        file_request(&mut world, firm, 3, PolicyKind::PermitAcceleration).expect("first request");
        // Five weeks pass the four-week decision, so the request is resolved but still cooling.
        for _ in 0..5 {
            world.tick = world.tick.next();
            weekly(&mut world);
        }
        assert!(world.firm(firm).unwrap().policy_requests[0].resolved);
        let repeat = file_request(&mut world, firm, 3, PolicyKind::TaxAdjustment);
        assert!(repeat.is_err(), "cooldown applies after the decision");
        // A different region has its own window.
        let other = file_request(&mut world, firm, 4, PolicyKind::TaxAdjustment);
        assert!(
            other.is_ok(),
            "another region is not cooling down: {other:?}"
        );
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
