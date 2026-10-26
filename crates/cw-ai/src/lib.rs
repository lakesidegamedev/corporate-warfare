//! Rival policy: observations to commands through the common validator.
//!
//! Every firm evaluates urgent debt, strike and offer deadlines weekly. Strategic decisions are
//! spread across four stable cohorts (`handle % 4`), so each firm gets one strategic slot every
//! four weeks regardless of population. Each slot scores a bounded candidate list by its
//! archetype weights on [cost saving, growth, quality/brand, R&D, acquisition], sorts by score
//! then command kind and target ID, and attempts at most four legal actions. Observations are
//! the firm's own books and public closing data (marks, fair estimates, distress status); the AI
//! gets no hidden income or stock and never mutates the world directly. The scheduler has no
//! private state: cohort and slot derive from the saved tick and stable handles, so a reloaded
//! career proposes exactly the same commands.

use cw_core::ids::Handle;
use cw_data::{Archetype, Dataset};
use cw_sim::commands::{BargainingChoice, CommandIntent};
use cw_sim::state::{BargainState, DealState, FacilityStatus, FirmStatus, HolderId, World};

/// Most strategic actions one firm may attempt in its slot.
pub const STRATEGIC_ACTIONS: usize = 4;
/// Hard cap on a week's proposals across the whole population.
pub const WEEKLY_PROPOSALS: usize = 256;

/// One proposed action for the session to validate and apply.
#[derive(Clone, Debug)]
pub struct Proposal {
    pub actor: u64,
    pub intent: CommandIntent,
}

/// A scored strategic candidate. Score is the archetype-weighted projected 52-week net cash
/// benefit proxy, in cents.
struct Candidate {
    score: i64,
    target: u64,
    intent: CommandIntent,
}

/// True when the player investor ultimately controls the firm. Holding a few shares does not
/// make a rival the player's company.
pub fn player_controlled(world: &World, firm: Handle) -> bool {
    cw_sim::economy::ownership::ultimate_controller(world, firm) == Some(HolderId::Investor)
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
        if player_controlled(world, handle) || !firm.operative() {
            continue;
        }
        urgent(world, dataset, handle, &mut proposals);
        if handle.slot() % 4 != cohort {
            continue;
        }
        strategic(world, dataset, handle, &mut proposals);
    }
    proposals.truncate(WEEKLY_PROPOSALS);
    proposals
}

/// Four weeks of mandatory outflow plus principal due over the next thirteen weeks.
fn reserve_target(world: &World, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    cw_sim::economy::insolvency::mandatory_weekly_outflow(world, firm) * 4
        + entity
            .loans
            .iter()
            .filter_map(|handle| world.loans.get(*handle))
            .map(|loan| loan.weekly_principal_cents)
            .sum::<i64>()
            * 13
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
    // The rules' order: cut discretionary spend, then seek funded credit, then restructure.
    // Production is cut only when the plant loses money on every unit at full capacity; a
    // seasonal or start-up loss on a plant that covers its variable costs over a year is
    // carried with credit, not by idling the plant before its profitable season.
    // A cash squeeze needs an answer even while the books show a profit: principal repayments
    // do not pass through income.
    if entity.book.cash() < outflow * 12 && (recent_income < 0 || entity.book.cash() < outflow * 6)
    {
        if structural_contribution(world, dataset, firm) < 0 || demand_limited(world, dataset, firm)
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SetProductionTarget {
                    firm,
                    target_bps: 6_000,
                },
            });
        } else {
            let capacity = cw_sim::economy::credit::secured_capacity(world, firm)
                + cw_sim::economy::credit::unsecured_capacity(world, firm);
            let need = (outflow * 12 - entity.book.cash()).min(capacity);
            let open_loans = entity.loans.len();
            if need > 0 && open_loans < 3 {
                proposals.push(Proposal {
                    actor: firm.0,
                    intent: CommandIntent::RequestLoan {
                        firm,
                        amount_cents: need,
                    },
                });
            }
        }
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

    // Strike deadline: settle a demand the firm can pay while keeping its reserve, otherwise
    // counter at the 95 percent floor the union accepts.
    if let BargainState::Demanding { .. } = entity.union.state {
        let demand = entity.union.demand_cents;
        let affordable = entity.book.cash() > reserve_target(world, firm) + demand;
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::RespondBargaining {
                firm,
                choice: if affordable {
                    BargainingChoice::Accept
                } else {
                    BargainingChoice::Counter
                },
                offer_cents: demand * 95 / 100,
            },
        });
    }

    // Offer deadline: a bid held in competition review offers the divestiture remedy. The
    // validator refuses an infeasible remedy, and the review then rejects with a refund.
    for (deal, stored) in world.deals.iter() {
        if stored.buyer == firm && stored.state == DealState::Review && stored.remedy_share_bps == 0
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SubmitRemedy {
                    deal,
                    divest_share_bps: 5_100,
                },
            });
        }
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
}

/// The strategic slot for one firm in isolation, for inspection and tests.
pub fn strategic_for(world: &World, dataset: &Dataset, firm: Handle) -> Vec<Proposal> {
    let mut proposals = Vec::new();
    strategic(world, dataset, firm, &mut proposals);
    proposals
}

fn strategic(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let archetype = entity.archetype.unwrap_or(Archetype::CostLeader);
    let weights = archetype.weights();
    let weeks = entity.book.recent_weeks().len().max(1) as i64;
    let revenue: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.revenue)
        .sum::<i64>()
        / weeks;
    let reserve = reserve_target(world, firm);
    let mut candidates: Vec<Candidate> = Vec::new();

    // Pricing: use actual last four weeks of sell-through. Benefit proxy: a year of the price
    // change on the volume actually sold.
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
            let weekly_units = sold / 13;
            candidates.push(Candidate {
                score: weights[0] * (52 * weekly_units.max(1) * (target - price).abs()),
                target: product as u64,
                intent: CommandIntent::SetPrice {
                    firm,
                    product,
                    price_cents: target,
                },
            });
        }
    }

    // Recovery: a firm that is profitable again restores capacity and marketing instead of
    // staying permanently shrunk, which otherwise ratchets corridor demand down over years.
    let recent_income: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    if recent_income > 0
        && entity.book.cash() > reserve * 2
        && entity.policies.production_target_bps < 8_500
    {
        candidates.push(Candidate {
            score: weights[1] * (recent_income / weeks * 52),
            target: firm.0,
            intent: CommandIntent::SetProductionTarget {
                firm,
                target_bps: 8_500,
            },
        });
    }

    // Brand builders and retailers fund marketing, which is the only source of media revenue.
    // Three percent of trailing weekly revenue keeps the corridor's studios operating without
    // turning into a hidden subsidy.
    if weights[2] >= 3 || entity.industry == 7 || revenue > 0 {
        let budget = (revenue * 300 / 10_000).max(10_000);
        if entity.book.cash() > reserve + budget * 4 && entity.policies.marketing_budget_cents == 0
        {
            candidates.push(Candidate {
                score: weights[2] * (revenue * 52 / 100),
                target: firm.0,
                intent: CommandIntent::SetMarketingBudget {
                    firm,
                    amount_cents: budget,
                },
            });
        }
    }

    // Research when cash supports it.
    if weights[3] >= 2 && revenue > 0 {
        let budget = (revenue * 100 / 10_000).max(100_000);
        if entity.book.cash() > reserve + budget * 4 && entity.policies.research_budget_cents == 0 {
            candidates.push(Candidate {
                score: weights[3] * (revenue * 52 / 200),
                target: firm.0,
                intent: CommandIntent::SetResearchBudget {
                    firm,
                    amount_cents: budget,
                },
            });
        }
    }

    // Growth: build only when the existing plant is fully staffed, the firm is profitable and
    // cash clears twice the reserve plus capex. Building while understaffed split labour and
    // added fixed cost faster than revenue, which is how the corridor failed.
    let fully_staffed = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|facility| facility.status == FacilityStatus::Operating)
        .all(|facility| facility.last_staff_ratio_bps >= 9_000);
    let total_blocks: i64 = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .map(|facility| facility.blocks as i64)
        .sum();
    // A quarter of results first: a few good opening weeks are not a case for a second plant.
    // Then build the best strategy of the sector in the state whose imports show room for it.
    if weights[1] >= 3
        && entity.book.recent_weeks().len() >= 13
        && entity.facilities.len() < 4
        && total_blocks < 6
        && fully_staffed
        && recent_income > 0
        && let Some((recipe, region, npv)) = best_expansion(world, dataset, firm)
    {
        let capex = dataset.recipes[recipe as usize].capex_cents;
        if entity.book.cash() > reserve * 2 + capex {
            candidates.push(Candidate {
                score: weights[1] * npv,
                target: recipe as u64,
                intent: CommandIntent::BuildFacility {
                    firm,
                    region,
                    recipe,
                    blocks: 1,
                },
            });
        }
    }

    // Retool a plant stuck in glut to a strategy of the same sector whose output has room.
    if let Some((facility, recipe, gain)) = best_retool(world, dataset, firm) {
        let blocks = world
            .facilities
            .get(facility)
            .map(|f| f.blocks as i64)
            .unwrap_or(1);
        let cost = dataset.recipes[recipe as usize].capex_cents * blocks / 4;
        if entity.book.cash() > reserve + cost {
            candidates.push(Candidate {
                score: weights[0].max(weights[1]) * gain,
                target: facility.0,
                intent: CommandIntent::RetoolFacility { facility, recipe },
            });
        }
    }

    // Consolidators and vultures bid for a distressed peer: 25 percent over the public mark,
    // for just over half the votes, only if escrow and diligence leave the reserve intact and
    // the combined debt stays within 60 percent of combined assets.
    let bidding = world
        .deals
        .iter()
        .any(|(_, deal)| deal.buyer == firm && !deal.state.terminal());
    if weights[4] >= 5
        && !bidding
        && let Some(target) = distressed_target(world, firm)
        && let Some(bid) = acquisition_bid(world, firm, target, reserve)
    {
        candidates.push(bid);
    }

    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.intent.key().cmp(b.intent.key()))
            .then_with(|| a.target.cmp(&b.target))
    });
    proposals.extend(
        candidates
            .into_iter()
            .take(STRATEGIC_ACTIONS)
            .map(|candidate| Proposal {
                actor: firm.0,
                intent: candidate.intent,
            }),
    );
}

fn acquisition_bid(
    world: &World,
    buyer: Handle,
    target: Handle,
    reserve: i64,
) -> Option<Candidate> {
    let bidder = world.firms.get(buyer)?;
    let entity = world.firms.get(target)?;
    let weights = bidder
        .archetype
        .unwrap_or(Archetype::Consolidator)
        .weights();
    // A vulture anchors on the depressed traded mark; a consolidator on the fair estimate.
    let vulture = bidder.archetype == Some(Archetype::Vulture);
    let mark = if vulture {
        entity.shares.mark_cents
    } else {
        entity.shares.mark_cents.max(entity.shares.fair_cents)
    };
    if mark <= 0 {
        return None;
    }
    let price = mark + mark * 25 / 100;
    let eligible = entity.shares.eligible_votes();
    let held = entity.shares.qty(HolderId::Firm(buyer));
    let wanted = (eligible / 2 + 1 - held).max(0);
    if wanted == 0 {
        return None;
    }
    let escrow = cw_sim::economy::deals::escrow_required(price, wanted);
    let diligence = price.saturating_mul(wanted) / 200;
    if bidder.book.cash() < reserve + escrow + diligence {
        return None;
    }
    let debt = bidder.book.debt() + entity.book.debt();
    let assets = bidder.book.total_assets() + entity.book.total_assets();
    if assets <= 0 || debt * 100 > assets * 60 {
        return None;
    }
    // Benefit proxy: a tenth of the target's trailing annual revenue, less the premium paid
    // over a 20 percent haircut of the public fair estimate.
    // Published revenue for a listed target; a private one is judged on its public fair
    // estimate alone (a twentieth of it a year as the contribution proxy).
    let revenue: i64 = if entity.shares.ipo_done {
        let weeks = entity.book.recent_weeks().len().max(1) as i64;
        entity
            .book
            .recent_weeks()
            .iter()
            .map(|week| week.revenue)
            .sum::<i64>()
            / weeks
    } else {
        entity.shares.fair_cents.saturating_mul(eligible) / 20 / 52 * 10
    };
    let haircut = entity.shares.fair_cents * 80 / 100;
    let premium = (price - haircut).max(0).saturating_mul(wanted);
    let benefit = revenue * 52 / 10 - premium / 10;
    Some(Candidate {
        score: weights[4] * benefit.max(1),
        target: target.0,
        intent: CommandIntent::StartDiligence {
            buyer,
            target,
            kind: cw_sim::state::DealKind::SharePurchase,
            price_cents: price,
            max_shares: wanted,
        },
    })
}

/// The plant is limited by demand: revenue is under half of full-capacity revenue, or more than
/// four weeks of full production sits unsold in stock, so producing more only adds cost.
fn demand_limited(world: &World, dataset: &Dataset, firm: Handle) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    let mut weekly: std::collections::BTreeMap<u16, i64> = std::collections::BTreeMap::new();
    for facility in entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|facility| facility.status == FacilityStatus::Operating)
    {
        let recipe = &dataset.recipes[facility.recipe as usize];
        if let Some(output) = dataset.product_index.get(&recipe.output) {
            *weekly.entry(*output).or_insert(0) +=
                recipe.weekly_output_lots * facility.blocks as i64;
        }
    }
    // Services keep no stock: there, demand shows as revenue under half of what the plant
    // would earn at full capacity and the firm's own prices.
    let weeks = entity.book.recent_weeks().len().max(1) as i64;
    let revenue = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.revenue)
        .sum::<i64>()
        / weeks;
    let capacity: i64 = weekly
        .iter()
        .map(|(product, lots)| {
            let price = entity
                .policies
                .prices
                .get(product)
                .copied()
                .unwrap_or_else(|| dataset.reference_price(*product));
            lots * price
        })
        .sum();
    if capacity > 0 && revenue * 2 < capacity {
        return true;
    }
    weekly.iter().any(|(product, lots)| {
        let stock: i64 = entity
            .inventory
            .iter()
            .filter(|((held, _), _)| held == product)
            .map(|(_, lot)| lot.qty.lots())
            .sum();
        *lots > 0 && stock > lots * 4
    })
}

/// Weekly contribution of the firm's operating plant at full capacity and mean seasonal yield:
/// output at the firm's own prices, less inputs at reference prices and full staffing at its
/// own wage offers. Observable from the firm's own policies and the public recipe.
fn structural_contribution(world: &World, dataset: &Dataset, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|facility| facility.status == FacilityStatus::Operating)
        .map(|facility| {
            let recipe = &dataset.recipes[facility.recipe as usize];
            let blocks = facility.blocks as i64;
            let Some(output) = dataset.product_index.get(&recipe.output).copied() else {
                return 0;
            };
            let price = entity
                .policies
                .prices
                .get(&output)
                .copied()
                .unwrap_or_else(|| dataset.reference_price(output));
            let revenue = recipe.weekly_output_lots * blocks * price;
            let inputs: i64 = recipe
                .weekly_inputs_lots
                .iter()
                .filter_map(|(id, qty)| {
                    dataset
                        .product_index
                        .get(id)
                        .map(|product| qty * blocks * dataset.reference_price(*product))
                })
                .sum();
            let wages: i64 = (0..3)
                .map(|tier| {
                    recipe.workers_low_mid_high[tier] * blocks * entity.policies.wage_offer[tier]
                })
                .sum();
            revenue - inputs - wages
        })
        .sum()
}

/// Public price of a product in a state: the gateway import price where one quotes, else the
/// reference price. No rival sees another firm's costs or order book.
fn public_price(world: &World, dataset: &Dataset, product: u16, region: u8) -> i64 {
    world
        .gateways
        .iter()
        .find(|g| g.region == region && g.product == product && g.sell_price_cents > 0)
        .map(|g| g.sell_price_cents)
        .unwrap_or_else(|| dataset.reference_price(product))
}

/// Room to build: lots of the product the state imported per week, smoothed.
fn room(world: &World, product: u16, region: u8) -> i64 {
    world
        .gateways
        .iter()
        .filter(|g| g.region == region && g.product == product)
        .map(|g| g.smoothed_import_lots)
        .sum()
}

/// Weekly contribution of one block of a strategy in a state at public prices and the firm's
/// own wage offers, after maintenance and land.
fn block_contribution(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    recipe: u16,
    region: u8,
) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let template = &dataset.recipes[recipe as usize];
    let Some(output) = dataset.product_index.get(&template.output).copied() else {
        return 0;
    };
    let revenue = template.weekly_output_lots * public_price(world, dataset, output, region);
    let inputs: i64 = template
        .weekly_inputs_lots
        .iter()
        .filter_map(|(id, qty)| {
            dataset
                .product_index
                .get(id)
                .map(|p| qty * public_price(world, dataset, *p, region))
        })
        .sum();
    let wage_index = world.regions[region as usize].wage_index_bps as i64;
    let payroll: i64 = (0..3)
        .map(|tier| {
            template.workers_low_mid_high[tier]
                * dataset.baseline_weekly_wage_cents[tier]
                * wage_index
                / 10_000
        })
        .sum();
    let maintenance = template.weekly_maintenance(dataset.maintenance_annual_bps);
    let land = dataset.fixture_land_rent_cents_weekly
        * world.regions[region as usize].land_index_bps as i64
        / 10_000;
    let _ = entity;
    revenue - inputs - payroll - maintenance - land
}

/// The best new plant for the firm: a strategy of its primary (or secondary) sector in the state
/// with the most room for its output, where imports cover at least four fifths of a block's
/// output (planned capacity at most 1.25 times reachable unmet demand). Ranked by three years of
/// contribution net of capex and financing; only positive cases are offered.
fn best_expansion(world: &World, dataset: &Dataset, firm: Handle) -> Option<(u16, u8, i64)> {
    let entity = world.firms.get(firm)?;
    let sectors: Vec<u8> = std::iter::once(entity.industry)
        .chain(entity.secondary_industry)
        .collect();
    let mut best: Option<(i64, u16, u8)> = None;
    for (index, template) in dataset.recipes.iter().enumerate() {
        let Some(sector) = cw_sim::economy::operations::industry_index(&template.industry) else {
            continue;
        };
        if !sectors.contains(&sector) {
            continue;
        }
        let Some(output) = dataset.product_index.get(&template.output).copied() else {
            continue;
        };
        for region in 0..world.regions.len() as u8 {
            if room(world, output, region) * 5 < template.weekly_output_lots * 4 {
                continue;
            }
            // Three years of contribution, less capex and three years of financing at the policy
            // rate (docs/rules/FINANCE-LABOUR-AI.md, build candidate).
            let financing =
                template.capex_cents * world.macro_state.policy_rate_bps as i64 * 3 / 10_000;
            let npv = block_contribution(world, dataset, firm, index as u16, region) * 156
                - template.capex_cents
                - financing;
            if npv > 0 && best.map(|(b, _, _)| npv > b).unwrap_or(true) {
                best = Some((npv, index as u16, region));
            }
        }
    }
    best.map(|(npv, recipe, region)| (recipe, region, npv))
}

/// A plant in glut (four weeks of output unsold) switched to the same-sector strategy whose
/// output has room in its state and the best contribution gain over three years, net of the retool
/// cost (a quarter of the new strategy's capex per block).
fn best_retool(world: &World, dataset: &Dataset, firm: Handle) -> Option<(Handle, u16, i64)> {
    let entity = world.firms.get(firm)?;
    let mut best: Option<(i64, Handle, u16)> = None;
    for handle in &entity.facilities {
        let Some(plant) = world.facilities.get(*handle) else {
            continue;
        };
        if plant.status != FacilityStatus::Operating {
            continue;
        }
        let current = &dataset.recipes[plant.recipe as usize];
        let Some(output) = dataset.product_index.get(&current.output).copied() else {
            continue;
        };
        let weekly = current.weekly_output_lots * plant.blocks as i64;
        if entity.stock(output, plant.region).lots() <= weekly * 4 {
            continue;
        }
        let now = block_contribution(world, dataset, firm, plant.recipe, plant.region)
            * plant.blocks as i64;
        for (index, template) in dataset.recipes.iter().enumerate() {
            if template.industry != current.industry || index as u16 == plant.recipe {
                continue;
            }
            let Some(next_output) = dataset.product_index.get(&template.output).copied() else {
                continue;
            };
            if next_output == output
                || room(world, next_output, plant.region) * 5 < template.weekly_output_lots * 4
            {
                continue;
            }
            let then = block_contribution(world, dataset, firm, index as u16, plant.region)
                * plant.blocks as i64;
            let gain = (then - now) * 156 - template.capex_cents * plant.blocks as i64 / 4;
            if gain > 0 && best.map(|(b, _, _)| gain > b).unwrap_or(true) {
                best = Some((gain, *handle, index as u16));
            }
        }
    }
    best.map(|(gain, handle, recipe)| (handle, recipe, gain))
}

/// The lowest-ID distressed peer outside the buyer's and the player's groups with no bid
/// already open on it. Distress is public: a quarter of losses, or a traded mark more than 20
/// percent under the fair estimate. Past-due firms cannot be bid for, so they are excluded.
fn distressed_target(world: &World, buyer: Handle) -> Option<Handle> {
    let group = cw_sim::economy::deals::competition_group(world, buyer);
    let mut candidates: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            // Only a listed company publishes statements; a private one shows only its mark.
            let losses: i64 = if firm.shares.ipo_done {
                firm.book
                    .recent_weeks()
                    .iter()
                    .map(|week| week.net_income)
                    .sum()
            } else {
                0
            };
            let discounted = firm.shares.mark_cents * 100 < firm.shares.fair_cents * 80;
            !group.contains(handle)
                && !player_controlled(world, *handle)
                && firm.status.can_trade()
                && firm.operative()
                && (losses < 0 || discounted)
                && !world
                    .deals
                    .iter()
                    .any(|(_, deal)| deal.target == *handle && !deal.state.terminal())
                && !cw_sim::economy::ownership::would_create_cycle(world, buyer, *handle)
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

    #[test]
    fn the_corridor_has_thirty_rivals_across_all_five_archetypes() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 153, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let rivals: Vec<_> = world
            .firms
            .iter()
            .filter(|(handle, _)| !player_controlled(&world, *handle))
            .filter_map(|(_, firm)| firm.archetype)
            .collect();
        assert!(rivals.len() >= 30, "{} rivals", rivals.len());
        for archetype in Archetype::ALL {
            assert!(rivals.contains(&archetype), "{archetype:?} is present");
        }
    }

    #[test]
    fn every_firm_gets_one_strategic_slot_in_any_four_weeks() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 154, ScenarioKind::Founder, ScenarioMode::Sandbox);
        for (handle, _) in world.firms.iter() {
            let slots = (0..4u32)
                .filter(|week| handle.slot() % 4 == week % 4)
                .count();
            assert_eq!(slots, 1);
        }
    }

    #[test]
    fn a_strategic_slot_attempts_at_most_four_actions() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 155, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let mut session_like = world.clone();
        for _ in 0..30 {
            cw_sim::tick::advance(&mut session_like, &dataset);
        }
        world = session_like;
        for (handle, _) in world.firms.iter() {
            assert!(strategic_for(&world, &dataset, handle).len() <= STRATEGIC_ACTIONS);
        }
    }

    #[test]
    fn a_token_player_stake_does_not_freeze_a_rival() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 156, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let rival = world
            .firms
            .iter()
            .find(|(handle, firm)| !player_controlled(&world, *handle) && firm.operative())
            .map(|(handle, _)| handle)
            .unwrap();
        world.tick = cw_core::calendar::Tick(rival.slot() % 4);
        let facility = world.firm(rival).unwrap().facilities[0];
        world
            .facilities
            .get_mut(facility)
            .unwrap()
            .last_staff_ratio_bps = 7_000;
        let before: usize = propose(&world, &dataset)
            .iter()
            .filter(|proposal| proposal.actor == rival.0)
            .count();
        assert!(before > 0);
        world
            .firms
            .get_mut(rival)
            .unwrap()
            .shares
            .add(HolderId::Investor, 1, 0);
        world.firms.get_mut(rival).unwrap().shares.issued += 1;
        let after: usize = propose(&world, &dataset)
            .iter()
            .filter(|proposal| proposal.actor == rival.0)
            .count();
        assert_eq!(before, after);
    }

    #[test]
    fn a_reloaded_world_proposes_the_same_commands() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 157, ScenarioKind::Founder, ScenarioMode::Sandbox);
        for _ in 0..21 {
            cw_sim::tick::advance(&mut world, &dataset);
        }
        let bytes = postcard::to_allocvec(&world).unwrap();
        let reloaded: World = postcard::from_bytes(&bytes).unwrap();
        let first: Vec<String> = propose(&world, &dataset)
            .iter()
            .map(|p| format!("{p:?}"))
            .collect();
        let second: Vec<String> = propose(&reloaded, &dataset)
            .iter()
            .map(|p| format!("{p:?}"))
            .collect();
        assert_eq!(first, second);
    }

    #[test]
    fn a_strike_deadline_gets_an_answer_outside_the_strategic_slot() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 158, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let rival = world
            .firms
            .iter()
            .find(|(handle, firm)| !player_controlled(&world, *handle) && firm.operative())
            .map(|(handle, _)| handle)
            .unwrap();
        // A week that is not this firm's strategic slot.
        world.tick = cw_core::calendar::Tick(rival.slot() % 4 + 1);
        {
            let firm = world.firms.get_mut(rival).unwrap();
            firm.union.state = BargainState::Demanding {
                deadline: cw_core::calendar::Tick(world.tick.0 + 1),
            };
            firm.union.demand_cents = 1_000_000;
        }
        let answered = propose(&world, &dataset).iter().any(|proposal| {
            matches!(proposal.intent, CommandIntent::RespondBargaining { firm, .. } if firm == rival)
        });
        assert!(answered);
    }

    /// A rival that lost money last week and holds under six weeks of mandatory outflow.
    fn squeezed(seed_value: u64, industry: &str) -> (World, Dataset, Handle) {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(
            &dataset,
            seed_value,
            ScenarioKind::Founder,
            ScenarioMode::Sandbox,
        );
        let firm = world
            .firms
            .iter()
            .find(|(handle, firm)| {
                !player_controlled(&world, *handle)
                    && firm.operative()
                    && cw_sim::state::INDUSTRY_NAMES[firm.industry as usize] == industry
            })
            .map(|(handle, _)| handle)
            .unwrap();
        let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(&world, firm);
        let cash = world.firm(firm).unwrap().book.cash();
        // A seasonal week: eight tenths of full-capacity revenue, with costs above it.
        let capacity: i64 = world
            .firm(firm)
            .unwrap()
            .facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .map(|f| {
                let recipe = &dataset.recipes[f.recipe as usize];
                recipe.weekly_output_lots
                    * f.blocks as i64
                    * dataset.reference_price(dataset.product_index[&recipe.output])
            })
            .sum();
        let revenue = capacity * 8 / 10;
        let tick = world.tick;
        let entity = world.firms.get_mut(firm).unwrap();
        let burn = (cash - outflow * 3).max(1) + revenue;
        entity
            .book
            .commit(
                tick,
                cw_core::ledger::Txn::new()
                    .debit(cw_core::ledger::Account::Cash, revenue, "fixture sales")
                    .credit(cw_core::ledger::Account::Revenue, revenue, "fixture sales")
                    .credit(cw_core::ledger::Account::Cash, burn, "fixture loss")
                    .debit(
                        cw_core::ledger::Account::OperatingExpense,
                        burn,
                        "fixture loss",
                    ),
            )
            .unwrap();
        let _ = entity.book.close_week(tick);
        (world, dataset, firm)
    }

    fn proposals_for(world: &World, dataset: &Dataset, firm: Handle) -> Vec<CommandIntent> {
        propose(world, dataset)
            .into_iter()
            .filter(|proposal| proposal.actor == firm.0)
            .map(|proposal| proposal.intent)
            .collect()
    }

    #[test]
    fn a_seasonal_farm_short_of_cash_borrows_instead_of_idling_its_plant() {
        let (world, dataset, farm) = squeezed(171, "agriculture");
        assert!(structural_contribution(&world, &dataset, farm) > 0);
        assert!(!demand_limited(&world, &dataset, farm));
        let intents = proposals_for(&world, &dataset, farm);
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, CommandIntent::RequestLoan { .. }))
        );
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, CommandIntent::SetProductionTarget { .. }))
        );
    }

    #[test]
    fn a_demand_limited_firm_short_of_cash_cuts_production() {
        let (mut world, dataset, farm) = squeezed(172, "agriculture");
        // Eight weeks of unsold output in stock.
        let region = world.firm(farm).unwrap().region;
        let product = dataset.product_index["food_raw"];
        let weekly: i64 = world
            .firm(farm)
            .unwrap()
            .facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .map(|f| dataset.recipes[f.recipe as usize].weekly_output_lots * f.blocks as i64)
            .sum();
        world
            .firms
            .get_mut(farm)
            .unwrap()
            .stock_lot_mut(product, region)
            .receive(
                cw_core::quantity::MilliLots::from_lots(weekly * 8),
                cw_core::money::Cents(0),
            );
        assert!(demand_limited(&world, &dataset, farm));
        let intents = proposals_for(&world, &dataset, farm);
        assert!(
            intents
                .iter()
                .any(|i| matches!(i, CommandIntent::SetProductionTarget { .. }))
        );
    }

    #[test]
    fn a_contractor_does_not_expand_without_projects_waiting() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 173, ScenarioKind::Founder, ScenarioMode::Sandbox);
        for (handle, firm) in world.firms.iter() {
            if cw_sim::state::INDUSTRY_NAMES[firm.industry as usize] == "construction" {
                assert!(
                    best_expansion(&world, &dataset, handle).is_none(),
                    "construction is a project service with no imports, so no room to build into"
                );
            }
        }
    }

    #[test]
    fn a_private_companys_books_do_not_reach_rival_decisions() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 174, ScenarioKind::Founder, ScenarioMode::Sandbox);
        for _ in 0..20 {
            cw_sim::tick::advance(&mut world, &dataset);
            world.pending_pause = None;
        }
        let private = world
            .firms
            .iter()
            .find(|(handle, firm)| {
                !firm.shares.ipo_done && firm.operative() && !player_controlled(&world, *handle)
            })
            .map(|(handle, _)| handle)
            .unwrap();
        let before: Vec<String> = propose(&world, &dataset)
            .iter()
            .filter(|p| p.actor != private.0)
            .map(|p| format!("{p:?}"))
            .collect();
        // A $100m private loss in the statements only; cash and the public mark are unchanged.
        let tick = world.tick;
        let entity = world.firms.get_mut(private).unwrap();
        entity
            .book
            .commit(
                tick,
                cw_core::ledger::Txn::new()
                    .debit(
                        cw_core::ledger::Account::OperatingExpense,
                        10_000_000_000,
                        "hidden",
                    )
                    .credit(
                        cw_core::ledger::Account::ShareCapital,
                        10_000_000_000,
                        "hidden",
                    ),
            )
            .unwrap();
        let _ = entity.book.close_week(tick);
        let after: Vec<String> = propose(&world, &dataset)
            .iter()
            .filter(|p| p.actor != private.0)
            .map(|p| format!("{p:?}"))
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn rivals_build_the_strategy_and_state_that_imports_point_to() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 175, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let chemicals = world
            .firms
            .iter()
            .find(|(handle, firm)| {
                !player_controlled(&world, *handle)
                    && cw_sim::state::INDUSTRY_NAMES[firm.industry as usize]
                        == "chemical_industries"
            })
            .map(|(handle, _)| handle)
            .unwrap();
        for gateway in &mut world.gateways {
            gateway.smoothed_import_lots = 0;
        }
        assert!(
            best_expansion(&world, &dataset, chemicals).is_none(),
            "no room, no build"
        );
        let plastics = dataset.product_index["plastics"];
        let pa = dataset.regions.iter().position(|r| r.code == "PA").unwrap() as u8;
        let mut found = false;
        for gateway in &mut world.gateways {
            if gateway.product == plastics && gateway.region == pa {
                gateway.smoothed_import_lots = 1_000;
                found = true;
            }
        }
        assert!(found, "Pennsylvania has a plastics gateway");
        let (recipe, region, npv) =
            best_expansion(&world, &dataset, chemicals).expect("room to build");
        assert_eq!(dataset.recipes[recipe as usize].id, "plastics_plant");
        assert_eq!(region, pa);
        assert!(npv > 0);
    }
}
