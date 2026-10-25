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
    // Solvency first: a firm that cannot fund twelve weeks of mandatory outflow cuts its
    // production target so payroll and inputs fall with revenue.
    let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, firm);
    let recent_income: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    if recent_income < 0 && entity.book.cash() < outflow * 12 {
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::SetProductionTarget {
                firm,
                target_bps: 6_000,
            },
        });
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::SetMarketingBudget {
                firm,
                amount_cents: 0,
            },
        });
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::SetResearchBudget {
                firm,
                amount_cents: 0,
            },
        });
    }

    // Understaffed facilities: raise the offer modestly, bounded by the regional prevailing
    // wage. A weekly five percent raise compounded into a wage spiral and bankrupted the
    // corridor, so the AI may only close a real gap, never bid above 110 percent of prevailing.
    let understaffed = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .any(|facility| {
            facility.status == FacilityStatus::Operating && facility.last_staff_ratio_bps < 8_000
        });
    if understaffed {
        let prevailing = world.regions[entity.region as usize].labour[0].prevailing_wage_cents;
        let ceiling = prevailing * 110 / 100;
        let current = entity.policies.wage_offer[0];
        if current < ceiling {
            let wage = (current * 102 / 100).min(ceiling).max(current + 1);
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SetWageOffer {
                    firm,
                    tier: 0,
                    wage_cents: wage,
                },
            });
        }
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

    // Brand builders and retailers fund marketing, which is the only source of media revenue.
    if weights[2] >= 3 || entity.industry == 7 || revenue > 0 {
        // Marketing is a real revenue-funded demand channel for media. Five percent of trailing
        // weekly revenue is enough to keep the corridor's two studios operating without turning
        // into a hidden subsidy.
        let budget = (revenue * 100 / 10_000).max(10_000);
        if entity.book.cash() > reserve + budget * 4 && entity.policies.marketing_budget_cents == 0
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SetMarketingBudget {
                    firm,
                    amount_cents: budget,
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

    // Growth: build only when the existing plant is fully staffed, the firm is profitable and
    // cash clears twice the reserve. Building while understaffed split labour and added fixed
    // cost faster than revenue, which is how the corridor failed.
    let fully_staffed = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|facility| facility.status == FacilityStatus::Operating)
        .all(|facility| facility.last_staff_ratio_bps >= 9_000);
    let recent_profit: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    if weights[1] >= 3
        && entity.facilities.len() < 2
        && fully_staffed
        && recent_profit > 0
        && entity.book.cash() > reserve * 2
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
        if contribution > 0 && entity.book.cash() > reserve * 2 + capex {
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
        if template.industry == "financial"
            || template.industry != cw_sim::state::INDUSTRY_NAMES[entity.industry as usize]
        {
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
    fn wage_offers_never_chase_above_the_regional_ceiling() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 194, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handle = world
            .firms
            .iter()
            .find(|(_, firm)| !firm.facilities.is_empty())
            .map(|(handle, _)| handle)
            .unwrap();
        let region = world.firm(handle).unwrap().region;
        let prevailing = world.regions[region as usize].labour[0].prevailing_wage_cents;
        {
            let firm = world.firms.get_mut(handle).unwrap();
            firm.policies.wage_offer[0] = prevailing * 120 / 100;
        }
        let facility = world.firm(handle).unwrap().facilities[0];
        world
            .facilities
            .get_mut(facility)
            .unwrap()
            .last_staff_ratio_bps = 7_000;
        // Put the firm in this week's strategic cohort.
        world.tick = cw_core::calendar::Tick(handle.slot() % 4);
        let proposals = propose(&world, &dataset);
        assert!(proposals.iter().all(|proposal| {
            !matches!(proposal.intent, CommandIntent::SetWageOffer { firm, .. } if firm == handle)
        }));
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
