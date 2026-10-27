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
        // Production follows sell-through plant by plant in the weekly tick, the same planner
        // as the player's automatic production (cw_sim::economy::pricing::plan).
        reopen(world, dataset, handle, &mut proposals);
        // Aggressive archetypes reconsider strategy every second week, the rest every fourth.
        let aggressive = fights(world, firm.archetype);
        let due = if aggressive {
            handle.slot() % 2 == cohort % 2
        } else {
            handle.slot() % 4 == cohort
        };
        if !due {
            continue;
        }
        strategic(world, dataset, handle, &mut proposals);
        speculate(world, dataset, handle, &mut proposals);
    }
    proposals.truncate(WEEKLY_PROPOSALS);
    proposals
}

/// Goods bought into store when the world price is low and recovering are released when it is high
/// or turning down. Tied to the world price path and the business cycle: a family at least 10
/// percent under its day-one level, no longer falling by more than 2 percent over the quarter,
/// while the cycle is in recession or recovery, is a buy; a family 5 percent over day one, or
/// falling more than 5 percent over the quarter above its day-one level, or a boom, is a sell.
/// A rival only deals in goods its plants make or use, only with cash well over its reserve and
/// profits behind it, never over 6 percent of cash per order, and only into space it already has.
fn speculate(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    use cw_sim::state::CyclePhase;
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let phase = world.macro_state.cycle.phase;
    let reserve = reserve_target(world, firm);
    let recent_income: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    let uses = cw_sim::economy::storage::uses(world, dataset, entity);
    let mut touched: std::collections::BTreeSet<(u16, u8)> = std::collections::BTreeSet::new();
    for handle in &entity.facilities {
        let Some(plant) = world.facilities.get(*handle) else {
            continue;
        };
        let recipe = &dataset.recipes[plant.recipe as usize];
        if recipe.storage_class.is_some() {
            continue;
        }
        for (id, _) in recipe.outputs() {
            if let Some(code) = dataset.product_index.get(id) {
                touched.insert((*code, plant.region));
            }
        }
        for id in recipe.weekly_inputs_lots.keys() {
            if let Some(code) = dataset.product_index.get(id) {
                touched.insert((*code, plant.region));
            }
        }
    }
    for (product, region) in touched {
        let Some(class) = dataset.storage_class_of(product) else {
            continue;
        };
        if dataset.products[product as usize].kind != cw_data::ProductKind::Goods
            || dataset.products[product as usize].spoilage_bps_weekly >= 200
        {
            continue;
        }
        let Some(family) = world.world_prices.family_of(product) else {
            continue;
        };
        let level = family.index_bps;
        let trend = family.trend_bps();
        let held = entity.storage.held_of(product, region);
        if held.0 > 0
            && (level >= 10_500 || (trend <= -500 && level > 10_000) || phase == CyclePhase::Boom)
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SellFromStore {
                    firm,
                    product,
                    region,
                    lots: 0,
                },
            });
            continue;
        }
        let buy = level <= 9_000
            && trend >= -200
            && matches!(phase, CyclePhase::Recession | CyclePhase::Recovery);
        if !buy
            || held.0 > 0
            || !entity.storage.orders.is_empty()
            || entity.book.cash() < reserve * 2
            || recent_income <= 0
        {
            continue;
        }
        let reference = dataset.reference_price(product).max(1);
        let lots = (entity.book.cash() * 6 / 100 / reference).clamp(0, 300);
        // Only into space the firm already has: yard and owned capacity beyond what it holds.
        let spare = uses
            .iter()
            .find(|u| u.region == region && u.class == class)
            .map(|u| (u.capacity_milli() - u.used_milli).max(0) / 1_000)
            .unwrap_or(0);
        let lots = lots.min(spare);
        if lots < 10 {
            continue;
        }
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::BuyToStore {
                firm,
                product,
                region,
                lots,
                limit_bps: 10_000,
            },
        });
    }
}

/// Bring a mothballed plant back when its output pays at public prices, the corridor is
/// importing at least half a plant's output, and the restart leaves the cash reserve intact.
fn reopen(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    if !matches!(entity.status, FirmStatus::Operating) {
        return;
    }
    for plant_handle in &entity.facilities {
        let Some(plant) = world.facilities.get(*plant_handle) else {
            continue;
        };
        if plant.status != FacilityStatus::Mothballed {
            continue;
        }
        let template = &dataset.recipes[plant.recipe as usize];
        let Some(output) = dataset.product_index.get(&template.output).copied() else {
            continue;
        };
        let imported: i64 = world
            .gateways
            .iter()
            .filter(|g| g.product == output)
            .map(|g| g.smoothed_import_lots)
            .sum();
        let cost = cw_sim::economy::operations::reopen_cost(plant);
        if block_contribution(world, dataset, firm, plant.recipe, plant.region) > 0
            && (imported * 2 >= template.weekly_output_lots * plant.blocks as i64
                || (!template.by_products.is_empty()
                    && room_covers(world, dataset, template, plant.region, 2)))
            && entity.book.cash() - cost >= reserve_target(world, firm)
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::ReopenFacility {
                    facility: *plant_handle,
                },
            });
            return;
        }
    }
}

/// A rival stops building new plants at this many plants in total. Six full-size plants before
/// 1.2.0 cut plants to a fifth of their size, so thirty now.
const MAX_GROWTH_BLOCKS: i64 = 30;

/// Weeks a raided company is left alone by rival raiders.
const RAID_RESPITE_WEEKS: u32 = 26;

/// Cost leaders, consolidators and vultures fight for markets: they undercut, invade occupied
/// markets and reconsider strategy twice as often.
/// Whether a rival fights for share: on Easy none do, on Hard all do, on Standard the cost
/// leaders, consolidators and vultures.
fn fights(world: &World, archetype: Option<Archetype>) -> bool {
    match world.difficulty {
        cw_data::Difficulty::Easy => false,
        cw_data::Difficulty::Hard => true,
        cw_data::Difficulty::Standard => archetype.map(aggressive).unwrap_or(false),
    }
}

fn aggressive(archetype: Archetype) -> bool {
    let weights = archetype.weights();
    weights[0] >= 3 || weights[4] >= 5
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

    // Pricing: the shared sell-through rule (cw_sim::economy::pricing), one price per good.
    for product in cw_sim::economy::pricing::outputs(world, dataset, firm) {
        let price = entity
            .policies
            .prices
            .get(&product)
            .copied()
            .unwrap_or_else(|| dataset.reference_price(product));
        let mut target = cw_sim::economy::pricing::target_price(world, dataset, firm, product);
        // Cost leaders and vultures undercut the going price by 3 percent wherever their own cost
        // still leaves a 5 percent margin: a price war, never a sale at a loss.
        if weights[0] >= 3
            && let Some(cost) = cw_sim::economy::pricing::unit_cost(world, dataset, firm, product)
        {
            let going = cw_sim::economy::pricing::market_price(world, dataset, firm, product);
            let undercut = going * 97 / 100;
            let reference = dataset.reference_price(product).max(1);
            if undercut < target && undercut >= cost * 105 / 100 {
                target = undercut.clamp(reference / 2, reference * 2);
            }
        }
        if target != price {
            let weekly_sold = cw_sim::economy::pricing::weekly_sold(world, firm, product);
            candidates.push(Candidate {
                score: weights[0] * (52 * weekly_sold.max(1) * (target - price).abs()),
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
    // Cautious archetypes (growth weight 2) still enter where the gap is plain: the corridor
    // importing at least one full plant of the strategy's output.
    if weights[1] >= 2
        && entity.book.recent_weeks().len() >= 13
        && entity.facilities.len() < 4
        && total_blocks < MAX_GROWTH_BLOCKS
        && fully_staffed
        && recent_income > 0
        && let Some((recipe, region, npv)) =
            best_expansion(world, dataset, firm, if weights[1] >= 3 { 4 } else { 5 })
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

    // Raid a rival's division in a cell the firm also operates in, when the captured capacity
    // is worth more over three years than the campaign and the purchase price, weighted by
    // the chance of success. Brand builders, consolidators and vultures raid most.
    if let Some((target, region, industry, budget, value)) = best_raid(world, dataset, firm) {
        candidates.push(Candidate {
            score: weights[2].max(weights[4]) * value,
            target: target.0,
            intent: CommandIntent::RaidMarket {
                firm,
                target,
                region,
                industry,
                budget_cents: budget,
            },
        });
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
        for (id, lots) in recipe.outputs() {
            if let Some(output) = dataset.product_index.get(id) {
                *weekly.entry(*output).or_insert(0) += lots * facility.blocks as i64;
            }
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
            let revenue: i64 = recipe
                .outputs()
                .filter_map(|(id, lots)| {
                    let output = dataset.product_index.get(id).copied()?;
                    let price = entity
                        .policies
                        .prices
                        .get(&output)
                        .copied()
                        .unwrap_or_else(|| dataset.reference_price(output));
                    Some(lots * blocks * price)
                })
                .sum();
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

/// What a plant in `region` nets per lot of a good by replacing imports: the best importing
/// gateway's price less freight from the plant. None when nothing of the good is imported.
fn import_netback(world: &World, dataset: &Dataset, product: u16, region: u8) -> Option<i64> {
    world
        .gateways
        .iter()
        .filter(|g| g.product == product && g.smoothed_import_lots > 0 && g.sell_price_cents > 0)
        .map(|g| {
            g.sell_price_cents
                - cw_sim::economy::freight::freight_cost(
                    dataset,
                    region,
                    g.region,
                    cw_core::quantity::MilliLots::from_lots(100),
                ) / 100
        })
        .max()
}

/// Room to build: lots of the product imported per week, smoothed. Goods trade on one corridor
/// exchange, so any region's plant can replace any region's imports and the room is the whole
/// corridor's; other products clear where they are made, so the room is the region's own.
fn room(world: &World, dataset: &Dataset, product: u16, region: u8) -> i64 {
    let corridor = dataset.products[product as usize].kind == cw_data::ProductKind::Goods;
    world
        .gateways
        .iter()
        .filter(|g| g.product == product && (corridor || g.region == region))
        .map(|g| g.smoothed_import_lots)
        .sum()
}

/// Whether imports leave room for a block of a strategy: imports of what it makes cover at least
/// `fifths` fifths of its output. A plant with by-products counts every product it makes at its
/// reference value, each capped at the plant's own yield of it, and needs three quarters of that
/// share, because a by-product with no import can still be exported or used by a sister plant.
fn room_covers(
    world: &World,
    dataset: &Dataset,
    template: &cw_data::Recipe,
    region: u8,
    fifths: i64,
) -> bool {
    if template.by_products.is_empty() {
        let Some(output) = dataset.product_index.get(&template.output).copied() else {
            return false;
        };
        return room(world, dataset, output, region) * 5 >= template.weekly_output_lots * fifths;
    }
    let (mut covered, mut total) = (0i128, 0i128);
    for (id, lots) in template.outputs() {
        let Some(product) = dataset.product_index.get(id).copied() else {
            continue;
        };
        let price = dataset.reference_price(product) as i128;
        covered += price * (room(world, dataset, product, region).min(lots)) as i128;
        total += price * lots as i128;
    }
    total > 0 && covered * 5 * 4 >= total * fifths as i128 * 3
}

/// Full-capacity weekly revenue of one block of a strategy at public prices in a state, every
/// product it makes counted.
fn block_revenue(world: &World, dataset: &Dataset, template: &cw_data::Recipe, region: u8) -> i64 {
    template
        .outputs()
        .filter_map(|(id, lots)| {
            dataset
                .product_index
                .get(id)
                .map(|product| lots * public_price(world, dataset, *product, region))
        })
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
    if !dataset.product_index.contains_key(&template.output) {
        return 0;
    }
    let revenue = block_revenue(world, dataset, template, region);
    let location = cw_sim::economy::sectors::location_factor_bps(
        dataset,
        entity.region,
        region,
        &template.industry,
    ) as i64;
    let inputs: i64 = template
        .weekly_inputs_lots
        .iter()
        .filter_map(|(id, qty)| {
            dataset
                .product_index
                .get(id)
                .map(|p| qty * public_price(world, dataset, *p, region))
        })
        .sum::<i64>()
        * location
        / 10_000;
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
fn best_expansion(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    room_fifths: i64,
) -> Option<(u16, u8, i64)> {
    let entity = world.firms.get(firm)?;
    let sectors: Vec<u8> = std::iter::once(entity.industry)
        .chain(entity.secondary_industry)
        .collect();
    let mut best: Option<(i64, u16, u8)> = None;
    for (index, template) in dataset.recipes.iter().enumerate() {
        let Some(sector) = cw_sim::economy::operations::industry_index(&template.industry) else {
            continue;
        };
        if !sectors.contains(&sector) || template.storage_class.is_some() {
            continue;
        }
        if !dataset.product_index.contains_key(&template.output) {
            continue;
        }
        let fighter = fights(world, entity.archetype);
        for region in 0..world.regions.len() as u8 {
            // An aggressive rival may invade a market the player holds even without unmet
            // demand, when it would still profit at prices a tenth lower in the fight to come.
            let invading = fighter
                && cw_sim::economy::sectors::division_strategy(
                    world, dataset, firm, region, sector,
                )
                .is_none()
                && cw_sim::economy::sectors::cell(world, dataset, region, sector)
                    .0
                    .iter()
                    .any(|(holder, _)| player_controlled(world, *holder));
            if !invading && !room_covers(world, dataset, template, region, room_fifths) {
                continue;
            }
            // A division runs one strategy: a different one in an existing division would need
            // a retool, which `best_retool` weighs separately.
            if cw_sim::economy::sectors::division_strategy(world, dataset, firm, region, sector)
                .is_some_and(|strategy| strategy != index as u16)
            {
                continue;
            }
            // Three years of contribution, less the real build cost (entry fee, founding
            // discount, dominance) and three years of financing at the policy rate
            // (docs/rules/FINANCE-LABOUR-AI.md, build candidate).
            let cost =
                cw_sim::economy::sectors::build_cost(world, dataset, firm, region, index as u16, 1)
                    .total_cents();
            let financing = cost * world.macro_state.policy_rate_bps as i64 * 3 / 10_000;
            // A good is sold where it is imported: value the output at what it nets there after
            // freight instead of at this state's own quote, so plants go near the demand.
            let netback: i64 = template
                .outputs()
                .filter_map(|(id, lots)| {
                    let product = dataset.product_index.get(id).copied()?;
                    if dataset.products[product as usize].kind != cw_data::ProductKind::Goods {
                        return None;
                    }
                    Some(
                        import_netback(world, dataset, product, region)
                            .map(|net| net - public_price(world, dataset, product, region))
                            .unwrap_or(0)
                            * lots,
                    )
                })
                .sum();
            let price_war = if invading {
                // A tenth off the output's price for the fight.
                block_revenue(world, dataset, template, region) / 10
            } else {
                0
            };
            let mut npv = (block_contribution(world, dataset, firm, index as u16, region)
                + netback
                - price_war)
                * 156
                - cost
                - financing;
            // Aggressive rivals prefer to fight for a market someone already holds, and the
            // player's markets most of all: invading makes the cell contested and raidable.
            if npv > 0 && fights(world, entity.archetype) {
                let (occupants, _) = cw_sim::economy::sectors::cell(world, dataset, region, sector);
                let rivals: Vec<Handle> = occupants
                    .iter()
                    .map(|(holder, _)| *holder)
                    .filter(|holder| !cw_sim::economy::ownership::same_group(world, *holder, firm))
                    .collect();
                if !rivals.is_empty() {
                    npv += npv / 4;
                }
                if rivals
                    .iter()
                    .any(|holder| player_controlled(world, *holder))
                {
                    npv += npv / 4;
                }
            }
            if npv > 0 && best.map(|(b, _, _)| npv > b).unwrap_or(true) {
                best = Some((npv, index as u16, region));
            }
        }
    }
    best.map(|(npv, recipe, region)| (recipe, region, npv))
}

/// The best market raid for a firm: a rival division in a cell it shares, aiming for a
/// two-to-one campaign, with a positive expected value after the campaign and purchase price.
fn best_raid(world: &World, dataset: &Dataset, firm: Handle) -> Option<(Handle, u8, u8, i64, i64)> {
    use cw_sim::economy::sectors;
    let entity = world.firms.get(firm)?;
    let reserve = reserve_target(world, firm);
    let mut best: Option<(i64, Handle, u8, u8, i64)> = None;
    for division in sectors::divisions_of(world, dataset, firm) {
        let ready = entity
            .raid_ready
            .get(&(division.region, division.industry))
            .copied()
            .unwrap_or(0);
        if world.tick.0 < ready {
            continue;
        }
        let (firms, _) = sectors::cell(world, dataset, division.region, division.industry);
        for (rival, _) in firms {
            if rival == firm || cw_sim::economy::ownership::same_group(world, rival, firm) {
                continue;
            }
            // A company that was raided in the last half year is left to recover first.
            if world
                .raids
                .iter()
                .any(|raid| raid.defender == rival && raid.tick + RAID_RESPITE_WEEKS > world.tick.0)
            {
                continue;
            }
            let probe = sectors::raid_quote(
                world,
                dataset,
                firm,
                rival,
                division.region,
                division.industry,
                0,
            );
            let budget = probe.defence_cents * 2;
            let quote = sectors::raid_quote(
                world,
                dataset,
                firm,
                rival,
                division.region,
                division.industry,
                budget,
            );
            if quote.blocks_at_stake == 0
                || entity.book.cash() < reserve + budget + quote.price_cents
            {
                continue;
            }
            let gain = block_contribution(world, dataset, firm, division.strategy, division.region)
                * quote.blocks_at_stake as i64
                * 156
                - quote.price_cents;
            let value = gain * quote.chance_bps as i64 / 10_000 - budget;
            if value > 0 && best.map(|(b, ..)| value > b).unwrap_or(true) {
                best = Some((value, rival, division.region, division.industry, budget));
            }
        }
    }
    best.map(|(value, rival, region, industry, budget)| (rival, region, industry, budget, value))
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
        // A retool moves the whole division, so it is valued and costed over all its plants,
        // and waits until none of them is busy.
        let industry = cw_sim::economy::sectors::recipe_industry(dataset, plant.recipe);
        let division =
            cw_sim::economy::sectors::division_plants(world, dataset, firm, plant.region, industry);
        if division
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .any(|p| {
                !matches!(
                    p.status,
                    FacilityStatus::Operating | FacilityStatus::Mothballed
                )
            })
        {
            continue;
        }
        let division_blocks: i64 = division
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .map(|p| p.blocks as i64)
            .sum();
        let now =
            block_contribution(world, dataset, firm, plant.recipe, plant.region) * division_blocks;
        for (index, template) in dataset.recipes.iter().enumerate() {
            if template.industry != current.industry
                || index as u16 == plant.recipe
                || template.storage_class.is_some()
            {
                continue;
            }
            let Some(next_output) = dataset.product_index.get(&template.output).copied() else {
                continue;
            };
            let _ = next_output;
            if next_output == output || !room_covers(world, dataset, template, plant.region, 4) {
                continue;
            }
            let then = block_contribution(world, dataset, firm, index as u16, plant.region)
                * division_blocks;
            let gain = (then - now) * 156 - template.capex_cents * division_blocks / 4;
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
            // The player's companies are fair game: a weak or cheap player company draws bids.
            !group.contains(handle)
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
                    best_expansion(&world, &dataset, handle, 4).is_none(),
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
    fn rivals_buy_cheap_world_goods_into_store_and_release_when_dear() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 177, ScenarioKind::Guided, ScenarioMode::Sandbox);
        cw_sim::economy::world_prices::init(
            &mut world,
            &dataset,
            &cw_data::price_paths::PricePathFile::load(),
            "great-lakes-2000",
            cw_data::PricePath::Historical,
            177,
        );
        // A rival that makes or uses a good in a price family, flush with cash and profit.
        let (rival, product, region) = world
            .firms
            .iter()
            .filter(|(handle, firm)| !player_controlled(&world, *handle) && firm.operative())
            .find_map(|(handle, firm)| {
                firm.facilities
                    .iter()
                    .filter_map(|f| world.facilities.get(*f))
                    .flat_map(|plant| {
                        dataset.recipes[plant.recipe as usize]
                            .outputs()
                            .filter_map(|(id, _)| dataset.product_index.get(id).copied())
                            .map(|code| (code, plant.region))
                            .collect::<Vec<_>>()
                    })
                    .find(|(code, _)| {
                        world.world_prices.family_of(*code).is_some()
                            && dataset.products[*code as usize].spoilage_bps_weekly < 200
                            && dataset.products[*code as usize].kind == cw_data::ProductKind::Goods
                    })
                    .map(|(code, region)| (handle, code, region))
            })
            .expect("a rival with a priced good");
        let tick = world.tick;
        {
            let entity = world.firms.get_mut(rival).unwrap();
            entity
                .book
                .commit(
                    tick,
                    cw_core::ledger::Txn::new()
                        .debit(cw_core::ledger::Account::Cash, 5_000_000_000, "test")
                        .credit(
                            cw_core::ledger::Account::ShareCapital,
                            5_000_000_000,
                            "test",
                        )
                        .debit(cw_core::ledger::Account::Cash, 1, "test")
                        .credit(cw_core::ledger::Account::Revenue, 1, "test"),
                )
                .unwrap();
            let _ = entity.book.close_week(tick);
            entity.storage.orders.clear();
        }
        let family = world.world_prices.product_family[&product] as usize;
        let run = |world: &World| -> Vec<Proposal> {
            let mut proposals = Vec::new();
            speculate(world, &dataset, rival, &mut proposals);
            proposals
        };
        // Day-one prices and an expansion: nothing to do.
        assert!(run(&world).is_empty());
        // A low, steadying world price in a recession is a buy for a good the rival can store.
        world.world_prices.families[family].index_bps = 8_000;
        world.world_prices.families[family].recent = vec![8_050];
        world.macro_state.cycle.phase = cw_sim::state::CyclePhase::Recession;
        let uses = cw_sim::economy::storage::uses(&world, &dataset, world.firm(rival).unwrap());
        let class = dataset.storage_class_of(product).unwrap();
        let spare = uses
            .iter()
            .find(|u| u.region == region && u.class == class)
            .map(|u| (u.capacity_milli() - u.used_milli) / 1_000)
            .unwrap_or(0);
        let bought = run(&world);
        assert!(spare >= 10, "the test rival has yard to spare: {spare}");
        {
            assert!(
                bought.iter().any(|p| matches!(
                    p.intent,
                    CommandIntent::BuyToStore { product: x, .. } if x == product
                )),
                "a cheap, steadying good is bought into the yard: {bought:?}"
            );
        }
        // Holding it while the price is 10 percent over day one releases it.
        world.firms.get_mut(rival).unwrap().storage.held.insert(
            (product, region),
            cw_core::quantity::MilliLots::from_lots(50),
        );
        world.world_prices.families[family].index_bps = 11_000;
        world.macro_state.cycle.phase = cw_sim::state::CyclePhase::Expansion;
        let sold = run(&world);
        assert!(sold.iter().any(|p| matches!(
            p.intent,
            CommandIntent::SellFromStore { product: x, lots: 0, .. } if x == product
        )));
    }

    #[test]
    fn rivals_plan_and_build_a_multi_output_chain() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 176, ScenarioKind::Founder, ScenarioMode::Sandbox);
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
        // Imports of the refinery's whole slate, none of it alone enough to justify the plant by
        // its main product's volume, make the refinery the best build: every product counts.
        let slate = ["fuel", "gasoline", "jet_fuel", "petrochem_feedstock"];
        for gateway in &mut world.gateways {
            if slate
                .iter()
                .any(|id| dataset.product_index[*id] == gateway.product)
            {
                gateway.smoothed_import_lots = 400;
            }
        }
        let refinery = dataset.recipe_index["refinery"];
        let template = &dataset.recipes[refinery as usize];
        let region = world.firm(chemicals).unwrap().region;
        assert!(room_covers(&world, &dataset, template, region, 4));
        // The plant's revenue counts every product it makes, not the main output alone.
        let all: i64 = block_revenue(&world, &dataset, template, region);
        let main_only = template.weekly_output_lots
            * public_price(&world, &dataset, dataset.product_index["fuel"], region);
        assert!(
            all > main_only * 2,
            "by-products add to the plant's revenue"
        );
        let (recipe, _, npv) =
            best_expansion(&world, &dataset, chemicals, 4).expect("room to build");
        assert_eq!(dataset.recipes[recipe as usize].id, "refinery");
        assert!(npv > 0);
        // With nothing imported, the refinery is not a build.
        for gateway in &mut world.gateways {
            gateway.smoothed_import_lots = 0;
        }
        assert!(!room_covers(&world, &dataset, template, region, 4));
    }

    #[test]
    fn rivals_build_the_strategy_imports_point_to_at_the_best_net_site() {
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
            best_expansion(&world, &dataset, chemicals, 4).is_none(),
            "no room, no build"
        );
        let plastics = dataset.product_index["plastics"];
        let pa = dataset
            .regions
            .iter()
            .position(|r| r.code == "PA-CEN")
            .unwrap() as u8;
        let mut found = false;
        for gateway in &mut world.gateways {
            if gateway.product == plastics && gateway.region == pa {
                gateway.smoothed_import_lots = 1_000;
                found = true;
            }
        }
        assert!(found, "Pennsylvania has a plastics gateway");
        let (recipe, region, npv) =
            best_expansion(&world, &dataset, chemicals, 4).expect("room to build");
        assert_eq!(dataset.recipes[recipe as usize].id, "plastics_plant");
        assert!(npv > 0);
        // Plastics trade on the corridor exchange, so the plant may sit in any state that nets
        // more after freight to Pennsylvania; a site that nets less than Pennsylvania itself is
        // never chosen over it.
        let net_there = import_netback(&world, &dataset, plastics, region).unwrap();
        let net_pa = import_netback(&world, &dataset, plastics, pa).unwrap();
        assert!(
            net_there <= net_pa,
            "freight is charged against distant sites"
        );
        if region != pa {
            let site = block_contribution(&world, &dataset, chemicals, recipe, region)
                + (net_there - public_price(&world, &dataset, plastics, region))
                    * dataset.recipes[recipe as usize].weekly_output_lots;
            let home = block_contribution(&world, &dataset, chemicals, recipe, pa);
            assert!(
                site >= home,
                "a distant site wins only on lower costs after freight"
            );
        }
    }
}
