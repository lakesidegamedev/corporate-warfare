//! Rival policy: observations to commands through the common validator.
//!
//! Every firm evaluates urgent debt, strike and offer deadlines weekly. Strategic decisions are
//! spread across four stable cohorts (`handle % 4`), so each firm gets one strategic slot every
//! four weeks regardless of population. Each slot examines a bounded shortlist and attempts at
//! most four legal actions. Observations are public closing data; the AI gets no hidden income
//! or stock and never mutates the world directly.

use cw_core::ids::Handle;
use cw_data::{Archetype, Dataset};
use cw_sim::commands::CommandIntent;
use cw_sim::state::{FacilityStatus, FirmStatus, HolderId, World};

/// One proposed action for the session to validate and apply.
#[derive(Clone, Debug)]
pub struct Proposal {
    pub actor: u64,
    pub intent: CommandIntent,
}

/// Produce this week's bounded command proposals in stable order.
pub fn propose(world: &World, dataset: &Dataset) -> Vec<Proposal> {
    let mut proposals = Vec::new();
    let mut handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    handles.sort_unstable();
    let cohort = world.tick.0 % 4;
    for handle in handles {
        let Some(firm) = world.firms.get(handle) else {
            continue;
        };
        // The investor's own firms are never operated by the rival AI.
        if firm.shares.qty(HolderId::Investor) > 0 {
            continue;
        }
        if !firm.operative() {
            continue;
        }
        urgent(world, dataset, handle, &mut proposals);
        let slot = handle.slot() % 4;
        if slot != cohort {
            continue;
        }
        strategic(world, dataset, handle, &mut proposals);
    }
    proposals.truncate(256);
    proposals
}

fn urgent(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    // A past-due firm preserves cash: stop research and marketing, and try legal credit.
    if matches!(
        entity.status,
        FirmStatus::PastDue { .. } | FirmStatus::Restructuring { .. }
    ) {
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::SetResearchBudget {
                firm,
                amount_cents: 0,
            },
        });
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::SetMarketingBudget {
                firm,
                amount_cents: 0,
            },
        });
        return;
    }
    // Understaffed facilities: raise the offered wage five percent.
    let understaffed = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .any(|facility| {
            facility.status == FacilityStatus::Operating && facility.last_staff_ratio_bps < 9_000
        });
    if understaffed {
        let wage = entity.policies.wage_offer[0] * 105 / 100;
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::SetWageOffer {
                firm,
                tier: 0,
                wage_cents: wage,
            },
        });
    }
    let _ = dataset;
}

fn strategic(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let archetype = entity.archetype.unwrap_or(Archetype::CostLeader);
    let weights = archetype.weights();
    let revenue: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.revenue)
        .sum::<i64>()
        / entity.book.recent_weeks().len().max(1) as i64;
    let reserve = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, firm) * 4
        + entity
            .loans
            .iter()
            .filter_map(|handle| world.loans.get(*handle))
            .map(|loan| loan.weekly_principal_cents)
            .sum::<i64>()
            * 13;

    // Pricing: use actual last four weeks of sell-through.
    for (region, product) in entity
        .last_week_units_sold
        .keys()
        .copied()
        .collect::<Vec<_>>()
    {
        let sold = entity.trailing_units(region, product);
        let stock = entity.stock(product, region).lots();
        let price = entity
            .policies
            .prices
            .get(&product)
            .copied()
            .unwrap_or_else(|| dataset.reference_price(product));
        let reference = dataset.reference_price(product).max(1);
        let mut target = price;
        if stock > sold.max(1) * 4 {
            target = price * 95 / 100;
        } else if sold >= stock.max(1) * 9 / 10 {
            target = price * 102 / 100;
        }
        target = target.clamp(reference / 2, reference * 2).max(1);
        if target != price {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SetPrice {
                    firm,
                    product,
                    price_cents: target,
                },
            });
        }
    }

    // Cost leader and technologist: fund research when cash supports it.
    if weights[3] >= 2 && revenue > 0 {
        let budget = (revenue * 100 / 10_000).max(100_000);
        if entity.book.cash() > reserve + budget * 4 {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SetResearchBudget {
                    firm,
                    amount_cents: budget,
                },
            });
        }
    }

    // Growth: build when the projected contribution is positive and cash clears the reserve.
    if weights[1] >= 3
        && let Some((recipe, region)) = cheapest_expansion(world, dataset, firm)
    {
        let template = &dataset.recipes[recipe as usize];
        let capex = template.capex_cents;
        let contribution = template.weekly_output_lots
            * dataset.reference_price(dataset.product_index[&template.output])
            - template
                .weekly_inputs_lots
                .iter()
                .map(|(id, qty)| qty * dataset.reference_price(dataset.product_index[id]))
                .sum::<i64>();
        if contribution > 0 && entity.book.cash() > reserve + capex {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::BuildFacility {
                    firm,
                    region,
                    recipe,
                    blocks: 1,
                },
            });
        }
    }

    // Consolidated or vulture: seek a funded acquisition of a distressed peer.
    if weights[4] >= 5
        && entity.book.cash() > reserve * 2
        && let Some(target) = distressed_target(world, firm)
    {
        let price = world
            .firms
            .get(target)
            .map(|f| f.shares.mark_cents)
            .unwrap_or(0);
        if price > 0 {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::StartDiligence {
                    buyer: firm,
                    target,
                    kind: cw_sim::state::DealKind::SharePurchase,
                    price_cents: price + price * 25 / 100,
                    max_shares: 550_000,
                },
            });
        }
    }
}

fn cheapest_expansion(world: &World, dataset: &Dataset, firm: Handle) -> Option<(u16, u8)> {
    let entity = world.firms.get(firm)?;
    let mut best: Option<(i64, u16, u8)> = None;
    for (index, template) in dataset.recipes.iter().enumerate() {
        if template.industry == "financial" {
            continue;
        }
        let region = entity.region;
        let cost = template.capex_cents
            + dataset.fixture_land_rent_cents_weekly
                * world.regions[region as usize].land_index_bps as i64
                / 10_000
                * 52;
        if best
            .map(|(best_cost, _, _)| cost < best_cost)
            .unwrap_or(true)
        {
            best = Some((cost, index as u16, region));
        }
    }
    best.map(|(_, recipe, region)| (recipe, region))
}

fn distressed_target(world: &World, buyer: Handle) -> Option<Handle> {
    let mut candidates: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            *handle != buyer
                && firm.shares.controller() != Some(HolderId::Investor)
                && matches!(
                    firm.status,
                    FirmStatus::PastDue { .. } | FirmStatus::Restructuring { .. }
                )
        })
        .map(|(handle, _)| handle)
        .collect();
    candidates.sort_unstable();
    candidates.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cw_sim::economy::seed;
    use cw_sim::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn proposals_are_bounded_and_deterministic() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 151, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let first = propose(&world, &dataset);
        let second = propose(&world, &dataset);
        assert_eq!(first.len(), second.len());
        assert!(first.len() <= 256);
    }

    #[test]
    fn investor_firms_are_never_operated_by_the_ai() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 152, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let proposals = propose(&world, &dataset);
        let primary = world.investor.primary_group.unwrap();
        assert!(proposals.iter().all(|proposal| proposal.actor != primary.0));
    }
}
