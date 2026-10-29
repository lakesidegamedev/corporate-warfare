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

mod selector;

pub use selector::{Class, style};

use std::collections::BTreeMap;

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
    let _memo = Memo::open();
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
        cw_sim::ph!("ai::urgent", urgent(world, dataset, handle, &mut proposals));
        // Production follows sell-through plant by plant in the weekly tick, the same planner
        // as the player's automatic production (cw_sim::economy::pricing::plan).
        cw_sim::ph!("ai::reopen", reopen(world, dataset, handle, &mut proposals));
        if wind_down(world, handle, &mut proposals) {
            continue;
        }
        // A state firm has one aim, covering its costs, which the pricing rules apply. It does
        // not expand, raid, speculate or lock lanes.
        if firm.state_owned_since.is_some() {
            continue;
        }
        // Aggressive archetypes reconsider strategy every second week, the rest every fourth.
        let aggressive = fights(world, dataset, handle);
        let due = if aggressive {
            handle.slot() % 2 == cohort % 2
        } else {
            handle.slot() % 4 == cohort
        };
        if !due {
            continue;
        }
        proposals.extend(cw_sim::ph!(
            "ai::slot_moves",
            slot_moves(world, dataset, handle)
        ));
    }
    proposals.truncate(WEEKLY_PROPOSALS);
    proposals
}

/// Everything a firm could do in its strategic slot, ranked by the selector: the strategic
/// candidates with their projected values, then the speculation, freight, structure and storage
/// generators, whose own economics decide what is on the table.
fn slot_moves(world: &World, dataset: &Dataset, firm: Handle) -> Vec<Proposal> {
    use selector::{Class, Move};
    let Some(entity) = world.firms.get(firm) else {
        return Vec::new();
    };
    // A middling firm's yearly turnover sets the scale of moves whose generator has no
    // projection of its own.
    let weeks = entity.book.recent_weeks();
    let turnover: i64 =
        weeks.iter().map(|w| w.revenue).sum::<i64>() * 52 / weeks.len().max(1) as i64;
    let scale = (turnover / 20).max(1_000_000);
    let mut moves: Vec<Move> = Vec::new();
    let push = |moves: &mut Vec<Move>, class: Option<Class>, ev: i64, proposal: Proposal| {
        let stance = selector::stance_for(world, firm, &proposal.intent);
        moves.push(Move {
            class,
            ev,
            stance,
            proposal,
        });
    };
    for candidate in cw_sim::ph!(
        "ai::strategic_candidates",
        strategic_candidates(world, dataset, firm)
    ) {
        let class = selector::classify(dataset, &candidate.intent);
        let proposal = Proposal {
            actor: firm.0,
            intent: candidate.intent,
        };
        if class.is_some() {
            push(&mut moves, class, candidate.score.max(1), proposal);
        } else {
            moves.push(Move::routine(proposal));
        }
    }
    let mut generated: Vec<Proposal> = Vec::new();
    cw_sim::ph!(
        "ai::speculate",
        speculate(world, dataset, firm, &mut generated)
    );
    cw_sim::ph!(
        "ai::logistics_moves",
        logistics_moves(world, dataset, firm, &mut generated)
    );
    cw_sim::ph!(
        "ai::structure_moves",
        structure_moves(world, dataset, firm, &mut generated)
    );
    cw_sim::ph!(
        "ai::storage_moves",
        storage_moves(world, dataset, firm, &mut generated)
    );
    for proposal in generated {
        let class = selector::classify(dataset, &proposal.intent);
        match class {
            Some(c) => push(&mut moves, Some(c), scale, proposal),
            None => moves.push(Move::routine(proposal)),
        }
    }
    if let Some(candidate) = cw_sim::ph!("ai::supplier_bid", supplier_bid(world, dataset, firm)) {
        let proposal = Proposal {
            actor: firm.0,
            intent: candidate.intent,
        };
        push(
            &mut moves,
            Some(Class::Integrate),
            candidate.score.max(1),
            proposal,
        );
    }
    cw_sim::ph!("ai::choose", selector::choose(world, dataset, firm, moves))
}

/// Structure verbs a rival uses on its own slot, weighted by its temperament
/// (`cw_sim::economy::structure::temperament`): borrow in the bond market when growth needs
/// cash, keep transfer prices honest toward minority holders or tilt them toward the lower-taxed
/// state, float a subsidiary when short of cash, and share a project too big to fund alone.
fn structure_moves(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    use cw_sim::economy::structure as st;
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let t = st::temperament(world, dataset, firm);
    let reserve = reserve_target(world, dataset, firm);
    let cash = entity.book.cash();
    let weeks = entity.book.recent_weeks();
    let income: i64 = weeks.iter().map(|w| w.net_income).sum();

    // Bonds: a profitable firm with thin cash and an open growth case borrows long and cheaply
    // when the price is sane. One issue at a time.
    if t.leverage >= 4_000
        && entity.bonds.is_empty()
        && weeks.len() >= 13
        && income > 0
        && cash < reserve * 3
        && entity.facilities.len() < 6
        && best_expansion(world, dataset, firm, 4).is_some_and(|(_, _, npv)| npv > 0)
    {
        let equity = entity.book.total_equity().max(0);
        let principal = (equity / 10).max(st::MIN_BOND_CENTS);
        let quote = st::bond_quote(world, firm, principal, 260);
        if quote.spread_bps <= 500
            && quote.coverage_bps >= 40_000
            && quote.leverage_bps <= 4_000
            && income > principal / 4
            && world
                .structure
                .profit_weeks
                .get(&firm)
                .copied()
                .unwrap_or(0)
                >= 39
            && entity.book.debt() + principal
                <= weeks.iter().map(|w| w.ebitda()).sum::<i64>() * 52 / (weeks.len().max(1) as i64)
                    * 3
            && principal <= quote.max_principal_cents
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::IssueBond {
                    firm,
                    principal_cents: principal,
                    coupon_bps: quote.coupon_bps + 25,
                    weeks: 260,
                },
            });
        }
    }

    // Transfer prices: the group's suppliers.
    let mut seen: std::collections::BTreeSet<(u8, u8)> = Default::default();
    for ((buyer, seller, _), link) in &world.structure.links {
        if *seller != firm || !seen.insert((link.region, link.industry)) {
            continue;
        }
        let current = st::transfer_bps(world, firm, link.region, link.industry);
        let minority = st::minority_bps(world, *seller).max(st::minority_bps(world, *buyer));
        let squeeze_year = world.structure.squeeze.get(seller).copied().unwrap_or(0)
            + world.structure.squeeze.get(buyer).copied().unwrap_or(0);
        let equity = entity.valuation.equity_cents.max(1);
        let want = if minority > 0 {
            // Never lean on minority holders; step back once the squeeze shows.
            if squeeze_year * 100 / equity >= 2 || current != st::TRANSFER_MARKET_BPS {
                st::TRANSFER_MARKET_BPS
            } else {
                current
            }
        } else {
            let gap = st::tax_rate_bps(world, *buyer) - st::tax_rate_bps(world, *seller);
            if t.tax_shifting >= 6_500 && gap >= 200 {
                11_000
            } else if t.tax_shifting >= 6_500 && gap <= -200 {
                9_000
            } else {
                st::TRANSFER_MARKET_BPS
            }
        };
        if want != current {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SetTransferPrice {
                    firm,
                    region: link.region,
                    industry: link.industry,
                    bps: want,
                },
            });
        }
    }

    // Listing: short of cash with a seasoned subsidiary worth floating.
    let carved_at = world.structure.carved.get(&firm).copied();
    if t.listing >= 4_500
        && (cash < reserve * 2 || carved_at.is_some_and(|week| world.tick.0 >= week + 26))
        && !entity.payout_freeze
    {
        let candidate = cw_sim::economy::ownership::controlled_by(world, HolderId::Firm(firm))
            .into_iter()
            .find(|sub| {
                world.firms.get(*sub).is_some_and(|f| {
                    f.operative()
                        && !f.shares.ipo_done
                        && f.shares.statement_weeks >= 13
                        && f.book.total_equity() > 0
                        && f.valuation.equity_cents > 300_000_000
                })
            });
        if let Some(sub) = candidate {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::ListSubsidiary {
                    firm: sub,
                    stake_bps: 3_000,
                },
            });
        }
    }

    // Capital-hungry with a big, healthy group: carve the largest division into its own
    // company, which the listing rule above floats once it has a quarter of results.
    if t.listing >= 5_000
        && income > 0
        && world.tick.0 >= 52
        && !world.structure.carved.contains_key(&firm)
        && cw_sim::economy::structure::minority_bps(world, firm) == 0
        && outside_float(world, firm) >= 2_500
        && cw_sim::economy::ownership::controlled_by(world, HolderId::Firm(firm)).is_empty()
    {
        let divisions = cw_sim::economy::sectors::divisions_of(world, dataset, firm);
        // A sprawling group (four divisions or more) unlocks value; a smaller one carves only
        // when short of cash.
        if divisions.len() >= 4
            && let Some(d) = divisions
                .iter()
                .max_by_key(|d| (d.blocks, d.region, d.industry))
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::CarveOutDivision {
                    firm,
                    region: d.region,
                    industry: d.industry,
                    name: format!(
                        "{} {}",
                        entity.name,
                        cw_sim::state::INDUSTRY_NAMES[d.industry as usize]
                    ),
                },
            });
        }
    }

    // Unlock value: a listed subsidiary the market prices under four fifths of its fair value is
    // handed to the parent's shareholders, who can hold it directly.
    if t.listing >= 4_500 && cw_sim::economy::structure::minority_bps(world, firm) >= 1_000 {
        let stuck = cw_sim::economy::ownership::controlled_by(world, HolderId::Firm(firm))
            .into_iter()
            .find(|sub| {
                world.firms.get(*sub).is_some_and(|f| {
                    f.operative()
                        && world
                            .structure
                            .carved
                            .get(&firm)
                            .is_some_and(|week| f.founding_week.0 >= *week)
                        && f.shares.ipo_done
                        && f.shares.fair_cents > 0
                        && (f.shares.mark_cents * 10 < f.shares.fair_cents * 6
                            || world.structure.loss_weeks.get(sub).copied().unwrap_or(0) >= 26)
                })
            });
        if let Some(sub) = stuck {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SpinOffSubsidiary { firm: sub },
            });
        }
    }

    // Joint venture: a growth project that costs more than half of what the firm can spend,
    // shared with a wealthier rival in the same state.
    let in_venture = world
        .structure
        .jvs
        .iter()
        .any(|j| j.a == firm || j.b == firm);
    if t.partnering >= 6_000
        && !in_venture
        && weeks.len() >= 13
        && income > 0
        && world.tick.0 >= 52
        && let Some((recipe, region, npv)) = best_expansion(world, dataset, firm, 4)
        && npv > 0
    {
        let cost = st::jv_capital(dataset, recipe, 1);
        let spare = cash - reserve * 2;
        let goods = dataset
            .product_index
            .get(&dataset.recipes[recipe as usize].output)
            .is_some_and(|p| dataset.products[*p as usize].kind == cw_data::ProductKind::Goods);
        if goods && spare * 3 > cost * 2 && spare < cost * 4 {
            let home_state = dataset.regions[entity.region as usize].state();
            let partner = world
                .firms
                .iter()
                .filter(|(h, f)| {
                    *h != firm
                        && f.operative()
                        && !player_controlled(world, *h)
                        && !cw_sim::economy::ownership::same_group(world, *h, firm)
                        && dataset.regions[f.region as usize].state() == home_state
                        && f.book.cash() > reserve_target(world, dataset, *h) * 2 + cost / 2
                        && cw_sim::economy::rivalry::grudge(world, firm, *h) < 5_000
                        && st::temperament(world, dataset, *h).partnering >= 3_000
                        && (feeds_a_plant(world, dataset, firm, recipe)
                            || feeds_a_plant(world, dataset, *h, recipe))
                })
                .map(|(h, f)| {
                    // Old partners are asked first, then the richest.
                    let tie = cw_sim::economy::rivalry::alliance(world, firm, h);
                    (tie >= 1_000, f.book.cash(), h)
                })
                .max()
                .map(|(_, cash, h)| (cash, h));
            if let Some((_, partner)) = partner {
                proposals.push(Proposal {
                    actor: firm.0,
                    intent: CommandIntent::FormJointVenture {
                        firm,
                        partner,
                        region,
                        recipe,
                        blocks: 1,
                        stake_bps: 5_000,
                        name: format!("{} Ventures {}", entity.name, world.tick.0),
                    },
                });
            }
        }
    }

    // A partner that wants out and a partner that can afford it: a quiet buyout after the lockup.
    for venture in &world.structure.jvs {
        if (venture.a != firm && venture.b != firm)
            || world.tick.0 < venture.formed + st::JV_LOCKUP_WEEKS + 26
        {
            continue;
        }
        let other = if venture.a == firm {
            venture.b
        } else {
            venture.a
        };
        let loss = trailing_income(world, venture.firm) < 0;
        if t.partnering >= 6_000
            && !loss
            && cash > reserve * 3 + cost_of_stake(world, venture.firm, other)
            && !player_controlled(world, other)
        {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::BuyOutVenture {
                    venture: venture.firm,
                    firm,
                },
            });
            break;
        }
        if loss && trailing_income(world, firm) < 0 && cash < reserve {
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::ExitVenture {
                    venture: venture.firm,
                    firm,
                },
            });
            break;
        }
    }
}

/// Whether one of the firm's own operating plants burns what the strategy makes: a joint
/// venture is only worth sharing when its output has a buyer inside the partnership.
fn feeds_a_plant(world: &World, dataset: &Dataset, firm: Handle, recipe: u16) -> bool {
    let output = &dataset.recipes[recipe as usize].output;
    world.firms.get(firm).is_some_and(|f| {
        f.facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .any(|p| {
                dataset.recipes[p.recipe as usize]
                    .weekly_inputs_lots
                    .contains_key(output)
            })
    })
}

fn outside_float(world: &World, firm: Handle) -> i32 {
    world
        .firms
        .get(firm)
        .map(|f| {
            f.shares
                .holders
                .keys()
                .filter(|h| h.is_external())
                .map(|h| f.shares.vote_bps(*h))
                .sum()
        })
        .unwrap_or(0)
}

fn trailing_income(world: &World, firm: Handle) -> i64 {
    world
        .firms
        .get(firm)
        .map(|f| f.book.recent_weeks().iter().map(|w| w.net_income).sum())
        .unwrap_or(0)
}

fn cost_of_stake(world: &World, venture: Handle, partner: Handle) -> i64 {
    world
        .firms
        .get(venture)
        .map(|f| {
            f.shares.qty(HolderId::Firm(partner))
                * f.shares.fair_cents.max(f.shares.mark_cents).max(1)
                * 11_000
                / 10_000
        })
        .unwrap_or(i64::MAX / 4)
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
    let reserve = reserve_target(world, dataset, firm);
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
        // The state reserve competes for the same goods: it sells into a spike, so a rival
        // releases its own store first, and it buys in a bust, which puts a floor under the
        // price that makes buying beside it safe.
        let reserve_phase = cw_sim::economy::reserve::phase(world, dataset, product);
        if held.0 > 0
            && (level >= 10_500
                || (trend <= -500 && level > 10_000)
                || phase == CyclePhase::Boom
                || reserve_phase == cw_sim::economy::reserve::Phase::Selling)
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
        // A speculator acts on a smaller dip and in any phase but a boom.
        let speculator = selector::style(world, dataset, firm) == cw_data::Temperament::Speculator;
        let buy = (level <= if speculator { 9_500 } else { 9_000 }
            && trend >= -200
            && (matches!(phase, CyclePhase::Recession | CyclePhase::Recovery)
                || (speculator && phase != CyclePhase::Boom)))
            || (reserve_phase == cw_sim::economy::reserve::Phase::Buying && level <= 9_500);
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

/// Freight: choose shipping policies, build carriers where lanes are short of capacity, and lock a
/// busy lane when that pays.
///
/// * A firm whose group owns carriers ships on them first; a brand builder takes the most reliable
///   route; everyone else takes the cheapest. One division is corrected per slot.
/// * A logistics firm that has earned for a quarter, has cash well over its reserve and owns fewer
///   than four sites builds on the lane with the most overflow to outside carriers, where a third
///   of the overflow priced at the lane rate over three years beats the build and its financing.
///   Rail and sea need a terminal or port at the lane's ends: the firm builds that first. A new
///   pipeline is pinned to the hottest truck lane from its region.
/// * An aggressive rival that owns at least a fifth of a lane and ships on it reserves up to two
///   fifths of what is still open for a quarter once the lane is three quarters full, when its
///   cash covers the fees: the lockout pushes other shippers onto dearer lanes.
fn logistics_moves(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    use cw_sim::economy::logistics as lg;
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let reserve = reserve_target(world, dataset, firm);
    let owns_carrier = world
        .logistics
        .carriers
        .iter()
        .any(|c| cw_sim::economy::ownership::same_group(world, firm, c.firm));
    let want = if owns_carrier {
        lg::ShipKind::OwnFirst
    } else if entity.archetype == Some(Archetype::BrandBuilder) {
        lg::ShipKind::Reliable
    } else {
        lg::ShipKind::Cheapest
    };
    let wrong = entity.facilities.iter().find_map(|handle| {
        let plant = world.facilities.get(*handle)?;
        let recipe = &dataset.recipes[plant.recipe as usize];
        (recipe.transport_mode.is_none()
            && plant.status == FacilityStatus::Operating
            && plant.ship.kind != want)
            .then_some(*handle)
    });
    if let Some(facility) = wrong {
        proposals.push(Proposal {
            actor: firm.0,
            intent: CommandIntent::SetShippingPolicy {
                facility,
                kind: want.code(),
                mode: None,
                carrier: None,
            },
        });
    }

    // A pipeline with no pinned lane goes where truck freight is heaviest from its region.
    for handle in &entity.facilities {
        let Some(plant) = world.facilities.get(*handle) else {
            continue;
        };
        let recipe = &dataset.recipes[plant.recipe as usize];
        if recipe.transport_mode.as_deref() != Some("pipeline")
            || plant.status != FacilityStatus::Operating
            || !plant.serves.is_empty()
        {
            continue;
        }
        let best = world
            .logistics
            .flows
            .iter()
            .filter(|(k, _)| k.mode == 0 && (k.a == plant.region || k.b == plant.region))
            .max_by_key(|(k, v)| (**v, std::cmp::Reverse(**k)));
        if let Some((key, _)) = best {
            let other = if key.a == plant.region { key.b } else { key.a };
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::SetCarrierLanes {
                    facility: *handle,
                    regions: vec![other],
                },
            });
        }
    }

    let sector_logistics = cw_sim::economy::operations::industry_index("logistics");
    let in_sector = sector_logistics
        .is_some_and(|s| entity.industry == s || entity.secondary_industry == Some(s));
    let recent_income: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    if in_sector
        && entity.book.recent_weeks().len() >= 13
        && recent_income > 0
        && entity.facilities.len() < 4
        && let Some((recipe, region)) = best_carrier_build(world, dataset, firm, reserve)
    {
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

    // A producer of liquids or gas builds a pipeline where its goods crowd a lane and it pays.
    if entity.book.recent_weeks().len() >= 13
        && recent_income > 0
        && entity.facilities.len() < 8
        && !entity.facilities.iter().any(|h| {
            world.facilities.get(*h).is_some_and(|p| {
                p.status == FacilityStatus::Building
                    && dataset.recipes[p.recipe as usize].transport_mode.as_deref()
                        == Some("pipeline")
            })
        })
        && let Some((recipe, region)) = pipeline_build(world, dataset, firm, reserve)
    {
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

    // Common carrier law forbids locking a lane up, so rivals do not try.
    if fights(world, dataset, firm) && !world.politics.laws.common_carrier {
        for (key, lane) in &world.logistics.lanes {
            if lane.fill_bps() < 7_500 || lg::group_share_bps(world, firm, *key) < 2_000 {
                continue;
            }
            let mine = lg::shipped_last_week(world, firm, *key) as i64;
            if mine < 50
                || world
                    .logistics
                    .contracts
                    .iter()
                    .any(|c| c.holder == firm && c.key == *key)
            {
                continue;
            }
            let open = lg::reservable_lots(world, firm, *key);
            let lots = (open * 40 / 100).min(mine * 2).min(i32::MAX as i64) as i32;
            if lots < 20 {
                continue;
            }
            let fee = lg::lane_rate_cents_per_lot(dataset, &world.logistics, *key) * lots as i64;
            if entity.book.cash() < reserve * 2 + fee * 13 {
                continue;
            }
            proposals.push(Proposal {
                actor: firm.0,
                intent: CommandIntent::ReserveLane {
                    firm,
                    from: key.a,
                    to: key.b,
                    mode: key.mode,
                    lots,
                    weeks: 13,
                },
            });
            break;
        }
    }
}

/// The carrier build with the best three-year value on a hot lane, or the terminal that lane
/// still needs: (recipe, region).
fn best_carrier_build(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    reserve: i64,
) -> Option<(u16, u8)> {
    use cw_sim::economy::logistics as lg;
    let entity = world.firms.get(firm)?;
    let logistics = cw_sim::economy::operations::industry_index("logistics")?;
    let mut best: Option<(i64, u16, u8)> = None;
    for hot in lg::hot_lanes(world, dataset).into_iter().take(8) {
        let def = &dataset.logistics.modes[hot.key.mode as usize];
        for region in [hot.key.a, hot.key.b] {
            // A division runs one strategy; a firm with another logistics strategy here skips it.
            let existing = cw_sim::economy::sectors::division_strategy(
                world, dataset, firm, region, logistics,
            );
            let access = lg::has_access(dataset, &world.logistics, hot.key.mode, region);
            for (index, recipe) in dataset.recipes.iter().enumerate() {
                if recipe.industry != "logistics"
                    || recipe.transport_mode.as_deref() != Some(def.id.as_str())
                {
                    continue;
                }
                // A rival does not guess where a pipeline would pay; those are for people.
                if def.needs_infrastructure {
                    continue;
                }
                // Without a terminal here the first build is the terminal, and only that.
                if def.needs_terminal && !access && !recipe.terminal {
                    continue;
                }
                if def.needs_terminal && access && recipe.terminal {
                    continue;
                }
                if existing.is_some_and(|s| s != index as u16) {
                    continue;
                }
                let cost = cw_sim::economy::sectors::build_cost(
                    world,
                    dataset,
                    firm,
                    region,
                    index as u16,
                    1,
                )
                .total_cents();
                if entity.book.cash() < reserve * 2 + cost {
                    continue;
                }
                let lots = hot.outside.min(recipe.weekly_output_lots as i32) as i64;
                // A third of the lane rate on the lots it can take over, three years of it.
                let take = if recipe.terminal { lots / 2 } else { lots };
                let npv = take * hot.rate_cents * 33 / 100 * 156
                    - cost
                    - cost * world.macro_state.policy_rate_bps as i64 * 3 / 10_000;
                if npv > 0 && best.as_ref().is_none_or(|b| npv > b.0) {
                    best = Some((npv, index as u16, region));
                }
            }
        }
    }
    best.map(|(_, recipe, region)| (recipe, region))
}

/// A pipeline for a producer of liquids or gas whose goods crowd a truck or rail lane. The
/// group's own liquid output there would ship at operating cost instead of the lane price, which
/// is the saving that pays for it; the rest of the pipeline's room sells to other shippers at the
/// lane rate, half of it counted. Only built where a lane is congested and the numbers clear
/// five years of financing, so a pipeline appears where it pays and nowhere else.
fn pipeline_build(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    reserve: i64,
) -> Option<(u16, u8)> {
    use cw_sim::economy::logistics as lg;
    let entity = world.firms.get(firm)?;
    let pipeline = dataset.logistics.mode_index("pipeline")?;
    let mut best: Option<(i64, u16, u8)> = None;
    for hot in lg::hot_lanes(world, dataset).into_iter().take(8) {
        if hot.key.mode == pipeline {
            continue;
        }
        for region in [hot.key.a, hot.key.b] {
            let own = liquid_output_lots(world, dataset, region, Some(firm));
            let all = liquid_output_lots(world, dataset, region, None);
            if own <= 0 {
                continue;
            }
            // The firm must ship on the lane today: a pipeline replaces freight it pays for.
            if lg::shipped_last_week(world, firm, hot.key) <= 0
                && !world
                    .logistics
                    .stats
                    .get(&hot.key)
                    .is_some_and(|s| s.users.iter().any(|(f, _)| *f == firm))
            {
                continue;
            }
            // An existing pipeline division of another strategy blocks a second one.
            for (index, recipe) in dataset.recipes.iter().enumerate() {
                if recipe.industry != "logistics"
                    || recipe.transport_mode.as_deref() != Some("pipeline")
                {
                    continue;
                }
                let logistics = cw_sim::economy::operations::industry_index("logistics")?;
                if cw_sim::economy::sectors::division_strategy(
                    world, dataset, firm, region, logistics,
                )
                .is_some_and(|s| s != index as u16)
                {
                    continue;
                }
                let cost = cw_sim::economy::sectors::build_cost(
                    world,
                    dataset,
                    firm,
                    region,
                    index as u16,
                    1,
                )
                .total_cents();
                if entity.book.cash() < reserve * 2 + cost {
                    continue;
                }
                let room = recipe.weekly_output_lots;
                let own_lots = own.min(room).min(hot.moved as i64);
                let other_lots = (all - own)
                    .max(0)
                    .min(room - own_lots)
                    .min(hot.moved as i64);
                let mut mult = lg::spot_mult_bps(dataset, 5_000);
                if world.politics.laws.common_carrier {
                    mult = mult.min(lg::regulated_cap_bps(dataset));
                }
                let pipe_rate =
                    lg::leg_cents(dataset, hot.key.a, hot.key.b, pipeline, 1, mult).max(1);
                let own_rate = pipe_rate * dataset.logistics.own_cost_bps / mult.max(1);
                let saving = (hot.rate_cents - own_rate).max(0);
                // A pipeline lasts for decades, so it is weighed over five years, not three.
                let npv = (own_lots * saving + other_lots * pipe_rate / 2) * 260
                    - cost
                    - cost * world.macro_state.policy_rate_bps as i64 * 5 / 10_000;
                if npv > 0 && best.as_ref().is_none_or(|b| npv > b.0) {
                    best = Some((npv, index as u16, region));
                }
            }
        }
    }
    best.map(|(_, recipe, region)| (recipe, region))
}

/// Weekly lots of liquid or gas goods the plants in a region make, for one firm's group or all.
fn liquid_output_lots(world: &World, dataset: &Dataset, region: u8, group: Option<Handle>) -> i64 {
    world
        .facilities
        .iter()
        .filter(|(_, plant)| plant.region == region && plant.status == FacilityStatus::Operating)
        .filter(|(_, plant)| {
            group.is_none_or(|g| cw_sim::economy::ownership::same_group(world, g, plant.firm))
        })
        .map(|(_, plant)| {
            dataset.recipes[plant.recipe as usize]
                .outputs()
                .filter(|(id, _)| {
                    dataset
                        .product_index
                        .get(*id)
                        .and_then(|p| dataset.storage_class_of(*p))
                        == Some(cw_data::StorageClass::Tank)
                })
                .map(|(_, lots)| lots * plant.blocks as i64)
                .sum::<i64>()
        })
        .sum()
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
            && entity.book.cash() - cost >= reserve_target(world, dataset, firm)
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

/// Weeks a rival company sits with nothing running before it winds itself down.
const IDLE_WIND_DOWN_WEEKS: u32 = 26;

/// A rival with nothing left to run winds itself down rather than bleed overheads: no plant
/// operating, building or retooling, no sales in its last thirteen closed weeks, and either every
/// plant mothballed for half a year or no plant at all after its first year. The sim's own
/// `wind_down_blocker` decides whether it may (solvent, no stakes, no unsettled bills), the same
/// test the player's command meets. Proposed outside the strategic slot, like reopening.
fn wind_down(world: &World, firm: Handle, proposals: &mut Vec<Proposal>) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    let now = world.tick.0;
    let plants: Vec<&cw_sim::state::Facility> = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .collect();
    let idle_long = if plants.is_empty() {
        now.saturating_sub(entity.founding_week.0) > 52
    } else {
        plants.iter().all(|plant| {
            plant.status == FacilityStatus::Mothballed
                && plant
                    .mothballed_since
                    .is_some_and(|since| now.saturating_sub(since.0) >= IDLE_WIND_DOWN_WEEKS)
        })
    };
    let weeks = entity.book.recent_weeks();
    let no_sales = weeks.len() >= 13 && weeks.iter().all(|week| week.revenue == 0);
    if !idle_long
        || !no_sales
        || proposals
            .iter()
            .any(|p| p.actor == firm.0 && matches!(p.intent, CommandIntent::ReopenFacility { .. }))
        || cw_sim::economy::insolvency::wind_down_blocker(world, firm).is_some()
    {
        return false;
    }
    proposals.push(Proposal {
        actor: firm.0,
        intent: CommandIntent::WindDown { firm },
    });
    true
}

/// A rival stops building new plants at this many plants in total. Six full-size plants before
/// 1.2.0 cut plants to a fifth of their size, so thirty now.
const MAX_GROWTH_BLOCKS: i64 = 30;

/// Weeks a raided company is left alone by rival raiders.
const RAID_RESPITE_WEEKS: u32 = 26;

/// Weeks a raider waits after launching a campaign before the next. Without it raids grew
/// fourfold over five years (each one leaves grudges that invite more) and the write-offs and
/// spent campaigns pushed the share of established companies losing money from 13 to 18 percent
/// between years 2 and 5; with it raids level off and that share goes from 12 to 16 percent
/// (16 seeds, five years, about 2 points of seed noise).
const RAID_COOLDOWN_WEEKS: u32 = 52;

/// Cost leaders, consolidators and vultures fight for markets: they undercut, invade occupied
/// markets and reconsider strategy twice as often.
/// Whether a rival fights for share: on Easy none do, on Hard all do, on Standard the cost
/// leaders, consolidators and vultures.
fn fights(world: &World, dataset: &Dataset, firm: Handle) -> bool {
    match world.difficulty {
        cw_data::Difficulty::Easy => false,
        cw_data::Difficulty::Hard => true,
        cw_data::Difficulty::Standard => {
            let archetype = world.firms.get(firm).and_then(|f| f.archetype);
            let weights =
                selector::blended_weights(archetype, selector::style(world, dataset, firm));
            archetype.is_some() && (weights[0] >= 3 || weights[4] >= 5)
        }
    }
}

/// Four weeks of mandatory outflow plus principal due over the next thirteen weeks.
fn reserve_target(world: &World, dataset: &Dataset, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let base = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, firm) * 4
        + entity
            .loans
            .iter()
            .filter_map(|handle| world.loans.get(*handle))
            .map(|loan| loan.weekly_principal_cents)
            .sum::<i64>()
            * 13;
    // The cautious hoard cash: half again on top of the rules' reserve.
    base * selector::thrift_pct(selector::style(world, dataset, firm)) / 100
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
                    lock: false,
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
        let affordable = entity.book.cash() > reserve_target(world, dataset, firm) + demand;
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
    let mut candidates = strategic_candidates(world, dataset, firm);
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

/// Every scored candidate of the strategic slot, unsorted and uncut.
fn strategic_candidates(world: &World, dataset: &Dataset, firm: Handle) -> Vec<Candidate> {
    let Some(entity) = world.firms.get(firm) else {
        return Vec::new();
    };
    let style = selector::style(world, dataset, firm);
    let weights = selector::blended_weights(entity.archetype, style);
    let weeks = entity.book.recent_weeks().len().max(1) as i64;
    let revenue: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.revenue)
        .sum::<i64>()
        / weeks;
    let reserve = reserve_target(world, dataset, firm);
    let mut candidates: Vec<Candidate> = Vec::new();

    // Prices follow the shared sell-through rule every week in the tick (pricing::weekly), so
    // a rival has no price move to schedule here.

    // Production follows sell-through plant by plant in the weekly tick, up as well as down, so
    // a profitable firm needs no whole-firm target reset: setting every plant back to 85 percent
    // fought the plan and made output swing between the two.
    let recent_income: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();

    // Brand builders and retailers fund marketing, which is the only source of media revenue.
    // Three percent of trailing weekly revenue keeps the corridor's studios operating without
    // turning into a hidden subsidy.
    if weights[2] >= 3 || entity.industry == 7 || revenue > 0 {
        let budget = (revenue * 300 / 10_000 * selector::marketing_pct(style) / 100).max(10_000);
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
        && !world.structure.jvs.iter().any(|j| j.firm == firm)
        && total_blocks < MAX_GROWTH_BLOCKS
        && fully_staffed
        && recent_income > 0
        && let Some((recipe, region, npv)) = best_expansion(
            world,
            dataset,
            firm,
            match weights[1] {
                5.. => 3,
                3..=4 => 4,
                _ => 5,
            },
        )
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
        && let Some(bid) = acquisition_bid(world, dataset, firm, target, reserve)
    {
        candidates.push(bid);
    }

    candidates
}

fn acquisition_bid(
    world: &World,
    dataset: &Dataset,
    buyer: Handle,
    target: Handle,
    reserve: i64,
) -> Option<Candidate> {
    let bidder = world.firms.get(buyer)?;
    let entity = world.firms.get(target)?;
    let weights = selector::blended_weights(
        Some(bidder.archetype.unwrap_or(Archetype::Consolidator)),
        selector::style(world, dataset, buyer),
    );
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
/// Gateway reads the planner repeats for every firm, recipe and region, kept for one `propose`
/// pass. The world is borrowed unchanged for the whole pass, so a remembered read is the read.
/// Outside a pass (tests, single helpers) every read is computed fresh.
#[derive(Default)]
struct Memo {
    room: BTreeMap<(u16, u8), i64>,
    public_price: BTreeMap<(u16, u8), i64>,
    import_netback: BTreeMap<(u16, u8), Option<i64>>,
    /// Holders of each (region, industry) cell with their shares.
    cells: BTreeMap<(u8, u8), Vec<(Handle, i64)>>,
}

thread_local! {
    static MEMO: std::cell::RefCell<Option<Memo>> = const { std::cell::RefCell::new(None) };
}

/// Clears the pass's memo when dropped, so no read outlives the world it came from.
struct MemoPass;

impl Memo {
    fn open() -> MemoPass {
        MEMO.with(|memo| *memo.borrow_mut() = Some(Memo::default()));
        MemoPass
    }

    fn read<K: Ord, T: Clone>(
        table: fn(&mut Memo) -> &mut BTreeMap<K, T>,
        key: K,
        compute: impl FnOnce() -> T,
    ) -> T {
        if let Some(hit) = MEMO.with(|memo| {
            memo.borrow_mut()
                .as_mut()
                .and_then(|m| table(m).get(&key).cloned())
        }) {
            return hit;
        }
        let value = compute();
        MEMO.with(|memo| {
            if let Some(m) = memo.borrow_mut().as_mut() {
                table(m).insert(key, value.clone());
            }
        });
        value
    }
}

impl Drop for MemoPass {
    fn drop(&mut self) {
        MEMO.with(|memo| *memo.borrow_mut() = None);
    }
}

/// The holders of a cell and their shares (`cw_sim::economy::sectors::cell`).
fn cell_holders(world: &World, dataset: &Dataset, region: u8, sector: u8) -> Vec<(Handle, i64)> {
    Memo::read(
        |m| &mut m.cells,
        (region, sector),
        || cw_sim::economy::sectors::cell(world, dataset, region, sector).0,
    )
}

fn public_price(world: &World, dataset: &Dataset, product: u16, region: u8) -> i64 {
    Memo::read(
        |m| &mut m.public_price,
        (product, region),
        || public_price_now(world, dataset, product, region),
    )
}

fn public_price_now(world: &World, dataset: &Dataset, product: u16, region: u8) -> i64 {
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
    Memo::read(
        |m| &mut m.import_netback,
        (product, region),
        || import_netback_now(world, dataset, product, region),
    )
}

fn import_netback_now(world: &World, dataset: &Dataset, product: u16, region: u8) -> Option<i64> {
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
    // A good's room is the corridor's, the same from every region.
    let key = (product, if corridor { u8::MAX } else { region });
    Memo::read(
        |m| &mut m.room,
        key,
        || room_now(world, product, region, corridor),
    )
}

fn room_now(world: &World, product: u16, region: u8, corridor: bool) -> i64 {
    world
        .gateways
        .iter()
        .filter(|g| g.product == product && (corridor || g.region == region))
        .map(|g| g.smoothed_import_lots)
        .sum()
}

/// Whether imports leave room for a block of a strategy: imports of what it makes cover at least
/// `fifths` fifths of its primary output. By-products do not enter the test: a by-product with no
/// import is still sold to the exchange, a sister plant or the outside buyer, so only the primary
/// good decides whether the corridor is short of what the plant is for. (Counting them diluted a
/// refinery's room to under half, because gasoline and jet fuel are made at home and never
/// imported, so no rival ever built one while fuel imports ran at six times local output.) Value
/// of the by-products still counts in `block_revenue`, so the build must pay.
fn room_covers(
    world: &World,
    dataset: &Dataset,
    template: &cw_data::Recipe,
    region: u8,
    fifths: i64,
) -> bool {
    let Some(output) = dataset.product_index.get(&template.output).copied() else {
        return false;
    };
    room(world, dataset, output, region) * 5 >= template.weekly_output_lots * fifths
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
    let fighter = fights(world, dataset, firm);
    // The world does not change while candidates are scored, so each cell is read once.
    let mut occupants_of: std::collections::BTreeMap<(u8, u8), Vec<Handle>> = Default::default();
    let mut strategy_of: std::collections::BTreeMap<(u8, u8), Option<u16>> = Default::default();
    for (index, template) in dataset.recipes.iter().enumerate() {
        let Some(sector) = cw_sim::economy::operations::industry_index(&template.industry) else {
            continue;
        };
        if !sectors.contains(&sector) || template.storage_class.is_some() {
            continue;
        }
        let Some(output) = dataset.product_index.get(&template.output).copied() else {
            continue;
        };
        for region in 0..world.regions.len() as u8 {
            // Under a production quota new extraction capacity would only sit idle.
            if world.politics.quota_active.contains(&(region, output)) {
                continue;
            }
            // An aggressive rival may invade a market the player holds even without unmet
            // demand, when it would still profit at prices a tenth lower in the fight to come.
            let strategy = *strategy_of.entry((region, sector)).or_insert_with(|| {
                cw_sim::economy::sectors::division_strategy(world, dataset, firm, region, sector)
            });
            let invading = fighter
                && strategy.is_none()
                && occupants_of
                    .entry((region, sector))
                    .or_insert_with(|| {
                        cell_holders(world, dataset, region, sector)
                            .into_iter()
                            .map(|(holder, _)| holder)
                            .collect()
                    })
                    .iter()
                    .any(|holder| player_controlled(world, *holder));
            if !invading && !room_covers(world, dataset, template, region, room_fifths) {
                continue;
            }
            // A division runs one strategy: a different one in an existing division would need
            // a retool, which `best_retool` weighs separately.
            if strategy.is_some_and(|strategy| strategy != index as u16) {
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
            // A good the corridor imports a quarter or more over what it makes is a market to
            // win back: home sellers already ask near the import, so a plant that replaces imports
            // is worth a quarter more than the same plant in a market the corridor supplies.
            if npv > 0
                && cw_sim::economy::pricing::import_dependence_bps(world, dataset, output)
                    .is_some_and(|bps| bps >= cw_sim::economy::pricing::IMPORT_DEPENDENT_BPS)
            {
                npv += npv / 4;
            }
            // Aggressive rivals prefer to fight for a market someone already holds, and the
            // player's markets most of all: invading makes the cell contested and raidable.
            if npv > 0 && fighter {
                let occupants = occupants_of.entry((region, sector)).or_insert_with(|| {
                    cell_holders(world, dataset, region, sector)
                        .into_iter()
                        .map(|(holder, _)| holder)
                        .collect()
                });
                let rivals: Vec<Handle> = occupants
                    .iter()
                    .copied()
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
    // One campaign a year: a raider that launched one in the last year waits.
    if world
        .raids
        .iter()
        .any(|raid| raid.attacker == firm && raid.tick + RAID_COOLDOWN_WEEKS > world.tick.0)
    {
        return None;
    }
    let reserve = reserve_target(world, dataset, firm);
    let mut best: Option<(i64, Handle, u8, u8, i64)> = None;
    for division in sectors::divisions_of(world, dataset, firm) {
        let firms = cell_holders(world, dataset, division.region, division.industry);
        for (rival, _) in firms {
            if rival == firm || cw_sim::economy::ownership::same_group(world, rival, firm) {
                continue;
            }
            // A friend is not raided.
            if cw_sim::economy::rivalry::alliance(world, firm, rival) >= 5_000 {
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
            // A new company or a newly opened division is left to find its feet.
            if world.tick.0
                < sectors::raid_open_tick(world, dataset, rival, division.region, division.industry)
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
                || entity.book.cash() < reserve + budget + quote.price_cents + quote.surcharge_cents
            {
                continue;
            }
            let gain = block_contribution(world, dataset, firm, division.strategy, division.region)
                * quote.blocks_at_stake as i64
                * 156
                - quote.price_cents;
            let value = gain * quote.chance_bps as i64 / 10_000 - budget - quote.surcharge_cents;
            // A grudge turns a marginal raid into a worthwhile one, and a raider's appetite for
            // the fight lets it take a raid that is a little short of paying for itself. Both
            // are bets on the same public numbers the validator sees; nothing is free.
            let grudge = cw_sim::economy::rivalry::grudge(world, firm, rival) as i64;
            let appetite =
                (selector::weight(selector::style(world, dataset, firm), Class::Raid) - 100).max(0)
                    * budget
                    / 400;
            let spite = if grudge >= 3_000 { budget / 2 } else { 0 };
            let value = if value > 0 {
                value * (100 + grudge / 40) / 100
            } else if value + appetite + spite > 0 {
                1
            } else {
                value
            };
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
        // A plant that is the corridor's only maker of one of its goods stays: retooling it would
        // leave that good, say the natural gas liquids a gas plant strips, with no producer.
        let sole_maker = current.outputs().any(|(id, _)| {
            !world.facilities.iter().any(|(other, p)| {
                other != *handle
                    && p.status == FacilityStatus::Operating
                    && dataset.recipes[p.recipe as usize].makes(id)
            })
        });
        if sole_maker {
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

/// A rival builds its own storage when the economics clear. The saving is real money only:
/// the outside fees on stock that overflows its space, four fifths of the spoilage a cold store
/// would stop, and for a speculator the swing it expects to trade into the space (a tenth of the
/// value of the stock it can afford, in a class where some good sits under its opening price).
/// Three years of that (five for a speculator, who keeps stores for the whole cycle) must beat
/// the build and its financing. Only temperaments that weight storage heavily (the integrator
/// and the speculator) consider it at all.
fn storage_moves(world: &World, dataset: &Dataset, firm: Handle, proposals: &mut Vec<Proposal>) {
    use cw_data::StorageClass;
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let t = selector::style(world, dataset, firm);
    if selector::weight(t, Class::Storage) < 150 {
        return;
    }
    let weeks = entity.book.recent_weeks();
    if weeks.len() < 13
        || weeks.iter().map(|w| w.net_income).sum::<i64>() <= 0
        || entity.facilities.len() >= 8
        || entity.facilities.iter().any(|h| {
            world.facilities.get(*h).is_some_and(|p| {
                p.status == FacilityStatus::Building
                    && dataset.recipes[p.recipe as usize].storage_class.is_some()
            })
        })
    {
        return;
    }
    let reserve = reserve_target(world, dataset, firm);
    let logistics = cw_sim::economy::operations::industry_index("logistics");
    let spoiled_year = entity.storage.recent(13).spoiled_cents * 4;
    let speculator = t == cw_data::Temperament::Speculator;
    let years: i64 = if speculator { 5 } else { 3 };
    let cash = entity.book.cash();
    // Every class the firm trades in, with what it overflows today.
    let mut classes: Vec<(u8, StorageClass, i64)> =
        cw_sim::economy::storage::uses(world, dataset, entity)
            .into_iter()
            .map(|u| (u.region, u.class, u.outside_milli() / 1_000))
            .collect();
    if speculator {
        // Space for a swing can be wanted where nothing overflows yet: its plants' own regions.
        for handle in &entity.facilities {
            let Some(plant) = world.facilities.get(*handle) else {
                continue;
            };
            for id in dataset.recipes[plant.recipe as usize]
                .weekly_inputs_lots
                .keys()
            {
                if let Some(code) = dataset.product_index.get(id)
                    && let Some(class) = dataset.storage_class_of(*code)
                    && !classes.iter().any(|c| c.0 == plant.region && c.1 == class)
                {
                    classes.push((plant.region, class, 0));
                }
            }
        }
    }
    let mut best: Option<(i64, u16, u8)> = None;
    for (region, class, outside_lots) in classes {
        let owned = cw_sim::economy::storage::uses(world, dataset, entity)
            .into_iter()
            .find(|u| u.region == region && u.class == class)
            .map(|u| u.owned_milli)
            .unwrap_or(0);
        // One store per class and region is enough for a rival.
        if owned > 0 {
            continue;
        }
        // The swing: the dearest good of this class sitting under its opening price.
        let swing_good = if speculator && cash > reserve * 4 {
            dataset
                .products
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    dataset.storage_class_of(*i as u16) == Some(class)
                        && p.spoilage_bps_weekly < 200
                        && world
                            .world_prices
                            .family_of(*i as u16)
                            .is_some_and(|f| f.index_bps <= 9_500)
                })
                .map(|(i, _)| dataset.reference_price(i as u16).max(1))
                .max()
        } else {
            None
        };
        for (index, recipe) in dataset.recipes.iter().enumerate() {
            if recipe.storage_class != Some(class) {
                continue;
            }
            if let Some(logistics) = logistics
                && cw_sim::economy::sectors::division_strategy(
                    world, dataset, firm, region, logistics,
                )
                .is_some_and(|s| s != index as u16)
            {
                continue;
            }
            let cost =
                cw_sim::economy::sectors::build_cost(world, dataset, firm, region, index as u16, 1)
                    .total_cents();
            if cash < reserve * 2 + cost {
                continue;
            }
            let capacity = recipe.storage_lots();
            let mut yearly = outside_lots.min(capacity) * class.outside_fee_cents() * 52;
            if class == StorageClass::Cold {
                yearly += spoiled_year * 80 / 100;
            }
            if let Some(price) = swing_good {
                let affordable = (cash - reserve * 2 - cost).max(0) * 15 / 100 / price;
                yearly += affordable.min(capacity) * price / 10;
            }
            let npv = yearly * years
                - cost
                - cost * world.macro_state.policy_rate_bps as i64 * years / 10_000;
            if npv > 0 && best.as_ref().is_none_or(|b| npv > b.0) {
                best = Some((npv, index as u16, region));
            }
        }
    }
    if let Some((_, recipe, region)) = best {
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

/// An integrator buys up its supply chain: the cheapest tradable peer outside its group whose
/// plants make a good the firm's own plants burn, bid at the usual premium.
fn supplier_bid(world: &World, dataset: &Dataset, firm: Handle) -> Option<Candidate> {
    let entity = world.firms.get(firm)?;
    if selector::weight(selector::style(world, dataset, firm), Class::Integrate) < 150
        || entity.book.recent_weeks().len() < 13
        || world
            .deals
            .iter()
            .any(|(_, d)| d.buyer == firm && !d.state.terminal())
    {
        return None;
    }
    let reserve = reserve_target(world, dataset, firm);
    let group = cw_sim::economy::deals::competition_group(world, firm);
    let mut best: Option<(i64, Handle)> = None;
    for (handle, peer) in world.firms.iter() {
        if group.contains(&handle)
            || !peer.operative()
            || !peer.status.can_trade()
            || peer.archetype.is_none()
            || peer.shares.mark_cents <= 0
            || cw_sim::economy::rivalry::alliance(world, firm, handle) >= 5_000
            || world
                .deals
                .iter()
                .any(|(_, d)| d.target == handle && !d.state.terminal())
            || cw_sim::economy::ownership::would_create_cycle(world, firm, handle)
        {
            continue;
        }
        // The peer must make something the firm burns.
        let supplies = peer
            .facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .filter(|p| p.status == FacilityStatus::Operating)
            .any(|p| {
                let output = &dataset.recipes[p.recipe as usize].output;
                entity
                    .facilities
                    .iter()
                    .filter_map(|h| world.facilities.get(*h))
                    .any(|own| {
                        dataset.recipes[own.recipe as usize]
                            .weekly_inputs_lots
                            .contains_key(output)
                    })
            });
        if !supplies {
            continue;
        }
        let key = peer.shares.mark_cents.max(1);
        if best.is_none_or(|(k, h)| (key, handle) < (k, h)) {
            best = Some((key, handle));
        }
    }
    let (_, target) = best?;
    acquisition_bid(world, dataset, firm, target, reserve)
}

/// A move's class, text for the dossier and whether it is aimed at a grudge, for the session to
/// record once the command is accepted.
#[derive(Clone, Debug)]
pub struct MoveNote {
    pub temperament: &'static str,
    pub class: &'static str,
    pub text: Option<String>,
    pub grudge: bool,
}

/// Describe a proposal for the move log. None for routine moves, which are not logged.
pub fn note(world: &World, dataset: &Dataset, proposal: &Proposal) -> Option<MoveNote> {
    let firm = Handle(proposal.actor);
    let mut class = selector::classify(dataset, &proposal.intent)?;
    let stance = selector::stance_for(world, firm, &proposal.intent);
    let name = |h: Handle| {
        world
            .firms
            .get(h)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "a former rival".to_owned())
    };
    let region = |r: u8| {
        world
            .regions
            .get(r as usize)
            .map(|x| x.name.clone())
            .unwrap_or_default()
    };
    let text = match &proposal.intent {
        CommandIntent::StartDiligence { target, .. } => {
            let supplier = world.firms.get(firm).is_some_and(|me| {
                world.firms.get(*target).is_some_and(|peer| {
                    peer.facilities
                        .iter()
                        .filter_map(|h| world.facilities.get(*h))
                        .any(|p| {
                            let out = &dataset.recipes[p.recipe as usize].output;
                            me.facilities
                                .iter()
                                .filter_map(|h| world.facilities.get(*h))
                                .any(|own| {
                                    dataset.recipes[own.recipe as usize]
                                        .weekly_inputs_lots
                                        .contains_key(out)
                                })
                        })
                })
            });
            if supplier {
                class = Class::Integrate;
            }
            Some(if supplier {
                format!("Bid for its supplier {}.", name(*target))
            } else {
                format!("Bid for {}.", name(*target))
            })
        }
        CommandIntent::IssueBond { .. } => Some("Issued bonds.".to_owned()),
        CommandIntent::WindDown { .. } => {
            Some("Wound itself down and returned its cash to shareholders.".to_owned())
        }
        CommandIntent::FormJointVenture {
            partner, region: r, ..
        } => Some(format!(
            "Formed a venture with {} in {}.",
            name(*partner),
            region(*r)
        )),
        CommandIntent::ListSubsidiary { .. } => Some("Floated a subsidiary.".to_owned()),
        CommandIntent::CarveOutDivision { .. } => Some("Carved a division out.".to_owned()),
        CommandIntent::SpinOffSubsidiary { .. } => Some("Spun off a subsidiary.".to_owned()),
        CommandIntent::ReserveLane { from, to, .. } => Some(format!(
            "Reserved freight space between {} and {}.",
            region(*from),
            region(*to)
        )),
        CommandIntent::BuildFacility {
            recipe, region: r, ..
        } => {
            let id = dataset.recipes.get(*recipe as usize)?.id.replace('_', " ");
            Some(match class {
                Class::Storage => format!("Started a {id} in {}.", region(*r)),
                Class::Carrier => format!("Started a {id} line in {}.", region(*r)),
                _ => format!("Started building a {id} plant in {}.", region(*r)),
            })
        }
        CommandIntent::BuyToStore { product, .. } => Some(format!(
            "Started buying {} for storage.",
            dataset.products[*product as usize].id.replace('_', " ")
        )),
        // Raids already appear in the dossier from the raid record.
        _ => None,
    };
    Some(MoveNote {
        temperament: selector::style(world, dataset, firm).key(),
        class: class.key(),
        text,
        grudge: class.hostile() && stance.grudge >= cw_sim::economy::rivalry::ACTIVE,
    })
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
    // A grudge picks the target; a friend is spared; otherwise the lowest handle.
    candidates.retain(|h| cw_sim::economy::rivalry::alliance(world, buyer, *h) < 5_000);
    candidates.sort_by_key(|h| {
        (
            std::cmp::Reverse(cw_sim::economy::rivalry::grudge(world, buyer, *h)),
            *h,
        )
    });
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
        // Fuel alone imported (gasoline and jet fuel are made at home and never imported) is
        // room for the plant, at the cautious five fifths as well.
        let fuel = dataset.product_index["fuel"];
        for gateway in &mut world.gateways {
            if gateway.product == fuel {
                gateway.smoothed_import_lots = 400;
            }
        }
        assert!(room_covers(&world, &dataset, template, region, 4));
        assert!(room_covers(&world, &dataset, template, region, 5));
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

    fn give_cash(world: &mut World, firm: Handle, cents: i64) {
        let tick = world.tick;
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .book
            .commit(
                tick,
                cw_core::ledger::Txn::new()
                    .debit(cw_core::ledger::Account::Cash, cents, "test")
                    .credit(cw_core::ledger::Account::ShareCapital, cents, "test"),
            )
            .unwrap();
    }

    fn add_carrier(
        world: &mut World,
        dataset: &Dataset,
        firm: Handle,
        region: u8,
        recipe: &str,
    ) -> Handle {
        let mut plant = world
            .facilities
            .iter()
            .map(|(_, f)| f.clone())
            .find(|f| f.firm == firm)
            .expect("the firm has a plant to copy");
        plant.region = region;
        plant.recipe = dataset.recipe_index[recipe];
        plant.blocks = 1;
        plant.status = FacilityStatus::Operating;
        plant.ship = Default::default();
        plant.serves = Vec::new();
        plant.last_output = cw_core::quantity::MilliLots::from_lots(
            dataset.recipes[plant.recipe as usize].weekly_output_lots,
        );
        let handle = world.facilities.insert(plant);
        world.firms.get_mut(firm).unwrap().facilities.push(handle);
        handle
    }

    #[test]
    fn a_liquids_producer_builds_a_pipeline_on_a_congested_lane_and_nowhere_else() {
        use cw_sim::economy::logistics as lg;
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 191, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let chem = world
            .firms
            .iter()
            .find(|(h, f)| f.name.starts_with("Scioto") && !player_controlled(&world, *h))
            .map(|(h, _)| h)
            .unwrap();
        let (a, b) = (
            dataset.region_index["OH-CEN"],
            dataset.region_index["OH-SE"],
        );
        // A big refinery in the congested lane's end region.
        let plant = add_carrier(&mut world, &dataset, chem, a, "refinery");
        world.facilities.get_mut(plant).unwrap().blocks = 12;
        give_cash(&mut world, chem, 50_000_000_000);
        let key = lg::LaneKey::new(a, b, 0);
        world.logistics.stats.insert(
            key,
            lg::LaneStat {
                moved: 3_000,
                outside: 2_500,
                hot_weeks: 6,
                users: vec![(chem, world.tick.0)],
                ..Default::default()
            },
        );
        let (recipe, region) = pipeline_build(&world, &dataset, chem, 0).expect("a pipeline pays");
        assert_eq!(
            dataset.recipes[recipe as usize].transport_mode.as_deref(),
            Some("pipeline")
        );
        assert!(region == a || region == b);
        // Not congested: no pipeline.
        world.logistics.stats.clear();
        assert!(pipeline_build(&world, &dataset, chem, 0).is_none());
        // A firm that makes no liquids there does not build one.
        world.logistics.stats.insert(
            key,
            lg::LaneStat {
                moved: 3_000,
                outside: 2_500,
                hot_weeks: 6,
                ..Default::default()
            },
        );
        let farm = world
            .firms
            .iter()
            .find(|(h, f)| f.industry == 0 && !player_controlled(&world, *h))
            .map(|(h, _)| h)
            .unwrap();
        give_cash(&mut world, farm, 50_000_000_000);
        assert!(pipeline_build(&world, &dataset, farm, 0).is_none());
    }

    #[test]
    fn rivals_ship_on_their_own_carriers_build_on_overflowing_lanes_and_lock_a_busy_one() {
        use cw_sim::economy::logistics as lg;
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 191, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let rivals: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(h, f)| !player_controlled(&world, *h) && f.operative() && f.industry != 6)
            .map(|(h, _)| h)
            .collect();
        let (owner, other) = (rivals[0], rivals[1]);
        let (a, b) = (
            dataset.region_index["OH-CEN"],
            dataset.region_index["OH-SE"],
        );
        add_carrier(&mut world, &dataset, owner, a, "carrier");
        add_carrier(&mut world, &dataset, other, a, "carrier");
        lg::begin_week(&mut world, &dataset);

        // The owner of a carrier ships on it first: its other divisions are told so.
        let mut proposals = Vec::new();
        logistics_moves(&world, &dataset, owner, &mut proposals);
        assert!(
            proposals
                .iter()
                .any(|p| matches!(p.intent, CommandIntent::SetShippingPolicy { kind: 2, .. })),
            "{proposals:?}"
        );

        // A logistics firm with cash builds a carrier where rail freight overflows to outside
        // carriers, and a terminal first where the lane's end has none.
        let oaken = world
            .firms
            .iter()
            .find(|(_, f)| f.name.starts_with("Oaken"))
            .map(|(h, _)| h)
            .unwrap();
        give_cash(&mut world, oaken, 50_000_000_000);
        let (chi, ne, ces) = (
            dataset.region_index["IL-CHI"],
            dataset.region_index["OH-NE"],
            dataset.region_index["IL-CEN"],
        );
        let rail = lg::LaneKey::new(chi, ne, 1);
        world.logistics.stats.insert(
            rail,
            lg::LaneStat {
                moved: 2_000,
                outside: 1_800,
                ..Default::default()
            },
        );
        let (recipe, region) = best_carrier_build(&world, &dataset, oaken, 0).expect("a build");
        assert_eq!(
            dataset.recipes[recipe as usize].transport_mode.as_deref(),
            Some("rail")
        );
        assert!(
            !dataset.recipes[recipe as usize].terminal,
            "both ends have yards"
        );
        assert!(region == chi || region == ne);
        let no_yard = lg::LaneKey::new(ces, ne, 1);
        world.logistics.stats.clear();
        world.logistics.stats.insert(
            no_yard,
            lg::LaneStat {
                moved: 2_000,
                outside: 1_800,
                ..Default::default()
            },
        );
        // Peoria has no rail yard, so the lane cannot exist yet and is not worth a carrier.
        assert!(!lg::lane_possible(&dataset, &world.logistics, 1, ces, ne));
        // Lanes that overflow in trucks need no terminal.
        world.logistics.stats.clear();
        world.logistics.stats.insert(
            lg::LaneKey::new(ces, ne, 0),
            lg::LaneStat {
                moved: 2_000,
                outside: 1_800,
                ..Default::default()
            },
        );
        let (recipe, _) = best_carrier_build(&world, &dataset, oaken, 0).expect("a truck build");
        assert_eq!(
            dataset.recipes[recipe as usize].transport_mode.as_deref(),
            Some("truck")
        );

        // An aggressive carrier that owns a fifth of a busy lane and ships on it reserves part of
        // what others could still use.
        world.difficulty = cw_data::Difficulty::Hard;
        give_cash(&mut world, owner, 50_000_000_000);
        let key = lg::LaneKey::new(a, b, 0);
        {
            let lane = world
                .logistics
                .lanes
                .get_mut(&key)
                .expect("both carriers serve the lane");
            for alloc in &mut lane.allocs {
                alloc.used = alloc.cap * 8 / 10;
            }
            lane.users.push((owner, 150));
            lane.users.push((other, 150));
        }
        let mut proposals = Vec::new();
        logistics_moves(&world, &dataset, owner, &mut proposals);
        let reserve = proposals.iter().find_map(|p| match p.intent {
            CommandIntent::ReserveLane {
                lots, weeks, mode, ..
            } => Some((lots, weeks, mode)),
            _ => None,
        });
        let (lots, weeks, mode) = reserve.expect("a lockout is proposed");
        assert!(lots >= 20 && weeks == 13 && mode == 0);
        // The proposal is legal: the validator accepts it.
        assert!(lg::reserve_lane(&mut world, &dataset, owner, a, b, 0, lots, weeks).is_ok());
        // The shipper that was squeezed out of the lane remembers who did it.
        assert!(cw_sim::economy::rivalry::grudge(&world, other, owner) > 0);
        assert_eq!(cw_sim::economy::rivalry::grudge(&world, owner, other), 0);
        assert_eq!(world.rivalry.stats.grudges_formed, 1);
    }

    // ----- CEO personality: selector, grudges, storage, integration -----

    fn person_with(dataset: &Dataset, style: cw_data::Temperament) -> u8 {
        dataset
            .cast
            .iter()
            .position(|p| p.temperament == style && p.can(cw_data::Role::Ceo))
            .expect("a chief executive of that temperament") as u8
    }

    fn rival_world(seed_value: u64) -> (World, Dataset, Handle) {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(
            &dataset,
            seed_value,
            ScenarioKind::Guided,
            ScenarioMode::Sandbox,
        );
        let rival = world
            .firms
            .handles_sorted()
            .into_iter()
            .find(|h| {
                !player_controlled(&world, *h) && world.firms.get(*h).unwrap().archetype.is_some()
            })
            .unwrap();
        (world, dataset, rival)
    }

    fn fake_move(firm: Handle, class: Class, stance: selector::Stance) -> selector::Move {
        selector::Move {
            class: Some(class),
            ev: 1_000_000_000,
            stance,
            proposal: Proposal {
                actor: firm.0,
                intent: CommandIntent::RaidMarket {
                    firm,
                    target: Handle(1),
                    region: 0,
                    industry: 0,
                    budget_cents: 1,
                },
            },
        }
    }

    /// How many of 400 weeks a move of this class survives the selector for this temperament.
    fn survival(
        world: &mut World,
        dataset: &Dataset,
        firm: Handle,
        style: cw_data::Temperament,
        class: Class,
        stance: selector::Stance,
    ) -> usize {
        world.firms.get_mut(firm).unwrap().ceo = Some(person_with(dataset, style));
        (1..=400u32)
            .filter(|week| {
                world.tick = cw_core::calendar::Tick(*week);
                !selector::choose(world, dataset, firm, vec![fake_move(firm, class, stance)])
                    .is_empty()
            })
            .count()
    }

    #[test]
    fn temperament_measurably_changes_which_moves_a_firm_makes() {
        use cw_data::Temperament::*;
        let (mut world, dataset, firm) = rival_world(5);
        let none = selector::Stance::default();
        let raider_raids = survival(&mut world, &dataset, firm, Raider, Class::Raid, none);
        let cautious_raids = survival(&mut world, &dataset, firm, Cautious, Class::Raid, none);
        let builder_builds = survival(&mut world, &dataset, firm, Builder, Class::Build, none);
        let cautious_builds = survival(&mut world, &dataset, firm, Cautious, Class::Build, none);
        let financier_bonds = survival(&mut world, &dataset, firm, Financier, Class::Bond, none);
        let builder_bonds = survival(&mut world, &dataset, firm, Builder, Class::Bond, none);
        let spec_store = survival(
            &mut world,
            &dataset,
            firm,
            Speculator,
            Class::Speculate,
            none,
        );
        let raider_store = survival(&mut world, &dataset, firm, Raider, Class::Speculate, none);
        assert_eq!(raider_raids, 400, "a raider always raids when it can");
        assert!(
            cautious_raids < 200,
            "the cautious mostly refrain: {cautious_raids}"
        );
        assert_eq!(builder_builds, 400);
        assert!(cautious_builds < builder_builds - 60);
        assert_eq!(financier_bonds, 400);
        assert!(builder_bonds < 400 || financier_bonds >= builder_bonds);
        assert_eq!(spec_store, 400);
        assert!(raider_store < spec_store);
    }

    #[test]
    fn a_grudge_sharpens_hostile_moves_and_an_ally_is_spared() {
        use cw_data::Temperament::*;
        let (mut world, dataset, firm) = rival_world(5);
        let calm = selector::Stance::default();
        let grudge = selector::Stance {
            grudge: 8_000,
            ally: 0,
        };
        let friend = selector::Stance {
            grudge: 0,
            ally: 6_000,
        };
        // A cautious firm rarely raids a stranger, far more often one that wronged it.
        let stranger = survival(&mut world, &dataset, firm, Cautious, Class::Raid, calm);
        let enemy = survival(&mut world, &dataset, firm, Cautious, Class::Raid, grudge);
        assert!(enemy > stranger * 2, "{enemy} vs {stranger}");
        // Nobody raids a friend.
        assert_eq!(
            survival(&mut world, &dataset, firm, Raider, Class::Raid, friend),
            0
        );
        // And utility rises with the grudge.
        world.firms.get_mut(firm).unwrap().ceo = Some(person_with(&dataset, Raider));
        let u = |s| selector::utility(&world, Raider, &fake_move(firm, Class::Raid, s), firm);
        assert!(u(grudge) > u(calm));
        assert!(u(friend) < u(calm));
        // Partnerships go the other way.
        let venture = |s| {
            let mut mv = fake_move(firm, Class::Venture, s);
            mv.proposal.intent = CommandIntent::ExitVenture {
                venture: Handle(1),
                firm,
            };
            selector::utility(&world, Raider, &mv, firm)
        };
        assert!(venture(friend) > venture(calm));
        assert_eq!(
            venture(selector::Stance {
                grudge: 6_000,
                ally: 0
            }),
            0
        );
    }

    #[test]
    fn difficulty_changes_aggression_and_sharpness_only() {
        use cw_data::Temperament::*;
        let (mut world, dataset, firm) = rival_world(5);
        let none = selector::Stance::default();
        let mut raids = Vec::new();
        for d in [
            cw_data::Difficulty::Easy,
            cw_data::Difficulty::Standard,
            cw_data::Difficulty::Hard,
        ] {
            world.difficulty = d;
            raids.push(survival(
                &mut world,
                &dataset,
                firm,
                Cautious,
                Class::Raid,
                none,
            ));
            // Peaceful moves are the same on every difficulty.
            assert!(survival(&mut world, &dataset, firm, Builder, Class::Build, none) == 400);
        }
        assert!(raids[0] < raids[1] && raids[1] < raids[2], "{raids:?}");
        // Sharpness: a keen firm sees a move's value exactly, a blunt one misjudges it.
        world.firms.get_mut(firm).unwrap().ceo = Some(person_with(&dataset, Builder));
        let values = |world: &mut World| -> std::collections::BTreeSet<i64> {
            (1..=60u32)
                .map(|week| {
                    world.tick = cw_core::calendar::Tick(week);
                    selector::utility(world, Builder, &fake_move(firm, Class::Build, none), firm)
                })
                .collect()
        };
        world.difficulty = cw_data::Difficulty::Hard;
        assert_eq!(values(&mut world).len(), 1);
        world.difficulty = cw_data::Difficulty::Easy;
        assert!(values(&mut world).len() > 10);
    }

    #[test]
    fn the_selector_is_deterministic_and_ranks_by_utility() {
        let (mut world, dataset, firm) = rival_world(5);
        world.firms.get_mut(firm).unwrap().ceo =
            Some(person_with(&dataset, cw_data::Temperament::Builder));
        world.tick = cw_core::calendar::Tick(37);
        let mut small = fake_move(firm, Class::Build, selector::Stance::default());
        small.ev = 1_000_000;
        let mut big = fake_move(firm, Class::Build, selector::Stance::default());
        big.ev = 900_000_000;
        big.proposal.intent = CommandIntent::RetoolFacility {
            facility: Handle(3),
            recipe: 1,
        };
        big.class = Some(Class::Retool);
        let a = selector::choose(&world, &dataset, firm, vec![small.clone(), big.clone()]);
        let b = selector::choose(&world, &dataset, firm, vec![big, small]);
        assert_eq!(a.len(), 2);
        assert_eq!(
            a.iter().map(|p| p.intent.key()).collect::<Vec<_>>(),
            b.iter().map(|p| p.intent.key()).collect::<Vec<_>>()
        );
        assert_eq!(a[0].intent.key(), "retool_facility", "bigger value first");
    }

    #[test]
    fn temperament_blends_into_the_archetype_weights() {
        use cw_data::Temperament::*;
        let w = |a, t| selector::blended_weights(Some(a), t);
        assert_eq!(w(Archetype::Consolidator, Financier)[4], 5);
        assert!(
            w(Archetype::Consolidator, Cautious)[4] < 5,
            "no bids from the cautious"
        );
        assert!(w(Archetype::Vulture, Builder)[4] < 5);
        assert!(w(Archetype::CostLeader, Builder)[1] >= 3, "builders build");
        assert!(
            w(Archetype::CostLeader, Raider)[2] >= 3,
            "raiders brand and raid"
        );
        assert_eq!(selector::thrift_pct(Cautious), 150);
        assert_eq!(selector::thrift_pct(Raider), 100);
    }

    fn fund_and_age(world: &mut World, firm: Handle, cash: i64) {
        let entity = world.firms.get_mut(firm).unwrap();
        for week in 0..14u32 {
            let tick = cw_core::calendar::Tick(week);
            entity
                .book
                .commit(
                    tick,
                    cw_core::ledger::Txn::new()
                        .debit(cw_core::ledger::Account::Cash, cash / 14 + 1_000, "test")
                        .credit(cw_core::ledger::Account::ShareCapital, cash / 14, "test")
                        .credit(cw_core::ledger::Account::Revenue, 1_000, "test"),
                )
                .unwrap();
            let _ = entity.book.close_week(tick);
        }
    }

    #[test]
    fn a_speculator_builds_a_store_ahead_of_a_swing_and_others_do_not() {
        use cw_data::Temperament::*;
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
        // A rival burning a priced, storable input.
        let (rival, product) = world
            .firms
            .iter()
            .filter(|(h, f)| !player_controlled(&world, *h) && f.operative())
            .find_map(|(h, f)| {
                f.facilities
                    .iter()
                    .filter_map(|p| world.facilities.get(*p))
                    .flat_map(|p| dataset.recipes[p.recipe as usize].weekly_inputs_lots.keys())
                    .filter_map(|id| dataset.product_index.get(id).copied())
                    .find(|code| {
                        world.world_prices.family_of(*code).is_some()
                            && dataset.products[*code as usize].spoilage_bps_weekly < 200
                            && dataset.storage_class_of(*code).is_some()
                    })
                    .map(|code| (h, code))
            })
            .expect("a rival with a priced input");
        fund_and_age(&mut world, rival, 80_000_000_000);
        let moves = |world: &World, style| {
            let mut w = world.clone();
            w.firms.get_mut(rival).unwrap().ceo = Some(person_with(&dataset, style));
            let mut out = Vec::new();
            storage_moves(&w, &dataset, rival, &mut out);
            out
        };
        let is_store = |p: &Proposal| match &p.intent {
            CommandIntent::BuildFacility { recipe, .. } => {
                dataset.recipes[*recipe as usize].storage_class.is_some()
            }
            _ => false,
        };
        // Day-one prices: nothing to swing, nothing overflowing.
        assert!(!moves(&world, Speculator).iter().any(is_store));
        let family = world.world_prices.product_family[&product] as usize;
        world.world_prices.families[family].index_bps = 8_000;
        let built = moves(&world, Speculator);
        assert!(
            built.iter().any(is_store),
            "a cheap good invites a store: {built:?}"
        );
        for style in [Raider, Cautious, Financier, Builder] {
            assert!(!moves(&world, style).iter().any(is_store), "{style:?}");
        }
    }

    #[test]
    fn an_integrator_bids_for_its_supplier_and_a_raider_does_not() {
        use cw_data::Temperament::*;
        let dataset = Dataset::load().unwrap();
        for seed_value in 1..40u64 {
            let mut world = seed::build(
                &dataset,
                seed_value,
                ScenarioKind::Guided,
                ScenarioMode::Sandbox,
            );
            let rivals: Vec<Handle> = world
                .firms
                .handles_sorted()
                .into_iter()
                .filter(|h| {
                    !player_controlled(&world, *h)
                        && world.firms.get(*h).unwrap().archetype.is_some()
                })
                .collect();
            let pair = rivals.iter().find_map(|&f| {
                rivals.iter().find_map(|&g| {
                    (f != g
                        && !cw_sim::economy::ownership::same_group(&world, f, g)
                        && feeds_between(&world, &dataset, g, f))
                    .then_some((f, g))
                })
            });
            let Some((buyer, supplier)) = pair else {
                continue;
            };
            fund_and_age(&mut world, buyer, 90_000_000_000);
            world.firms.get_mut(supplier).unwrap().shares.mark_cents = world
                .firms
                .get(supplier)
                .unwrap()
                .shares
                .mark_cents
                .max(1_000);
            world.firms.get_mut(buyer).unwrap().ceo = Some(person_with(&dataset, Integrator));
            let Some(bid) = supplier_bid(&world, &dataset, buyer) else {
                continue;
            };
            assert!(matches!(bid.intent, CommandIntent::StartDiligence { .. }));
            let note = note(
                &world,
                &dataset,
                &Proposal {
                    actor: buyer.0,
                    intent: bid.intent,
                },
            )
            .expect("a bid is logged");
            assert_eq!(note.class, "integrate");
            assert!(note.text.unwrap().starts_with("Bid for its supplier"));
            world.firms.get_mut(buyer).unwrap().ceo = Some(person_with(&dataset, Raider));
            assert!(supplier_bid(&world, &dataset, buyer).is_none());
            return;
        }
        panic!("no buyer and supplier pair in forty worlds");
    }

    fn feeds_between(world: &World, dataset: &Dataset, supplier: Handle, buyer: Handle) -> bool {
        let (Some(s), Some(b)) = (world.firms.get(supplier), world.firms.get(buyer)) else {
            return false;
        };
        s.facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .any(|p| {
                let out = &dataset.recipes[p.recipe as usize].output;
                b.facilities
                    .iter()
                    .filter_map(|h| world.facilities.get(*h))
                    .any(|q| {
                        dataset.recipes[q.recipe as usize]
                            .weekly_inputs_lots
                            .contains_key(out)
                    })
            })
    }

    #[test]
    fn a_grudge_picks_the_bid_target_and_a_friend_is_spared() {
        let (mut world, dataset, buyer) = rival_world(5);
        let others: Vec<Handle> = world
            .firms
            .handles_sorted()
            .into_iter()
            .filter(|h| {
                *h != buyer
                    && !player_controlled(&world, *h)
                    && world.firms.get(*h).unwrap().archetype.is_some()
                    && !cw_sim::economy::ownership::same_group(&world, buyer, *h)
            })
            .collect();
        for h in &others[..3] {
            let f = world.firms.get_mut(*h).unwrap();
            f.shares.fair_cents = 1_000;
            f.shares.mark_cents = 500;
        }
        let (first, last) = (others[0], others[2]);
        assert_eq!(distressed_target(&world, buyer), Some(first));
        cw_sim::economy::rivalry::note_grudge(
            &mut world,
            buyer,
            last,
            6_000,
            cw_sim::economy::rivalry::Cause::HostileBid,
        );
        assert_eq!(
            distressed_target(&world, buyer),
            Some(last),
            "the grudge picks"
        );
        cw_sim::economy::rivalry::note_alliance(
            &mut world,
            buyer,
            last,
            9_000,
            cw_sim::economy::rivalry::Cause::Venture,
        );
        assert_ne!(
            distressed_target(&world, buyer),
            Some(last),
            "a friend is spared"
        );
        let _ = &dataset;
    }

    #[test]
    fn moves_are_logged_with_text_for_the_dossier_and_routine_ones_are_not() {
        let (world, dataset, firm) = rival_world(5);
        let bond = Proposal {
            actor: firm.0,
            intent: CommandIntent::IssueBond {
                firm,
                principal_cents: 1,
                coupon_bps: 500,
                weeks: 260,
            },
        };
        let note_bond = note(&world, &dataset, &bond).expect("logged");
        assert_eq!(note_bond.class, "bond");
        assert_eq!(note_bond.text.as_deref(), Some("Issued bonds."));
        let price = Proposal {
            actor: firm.0,
            intent: CommandIntent::SetProductionTarget {
                firm,
                target_bps: 5_000,
                lock: false,
            },
        };
        assert!(note(&world, &dataset, &price).is_none());
    }

    #[test]
    fn a_world_with_grudges_still_reloads_into_the_same_proposals() {
        let (mut world, dataset, firm) = rival_world(9);
        let other = world
            .firms
            .handles_sorted()
            .into_iter()
            .find(|h| *h != firm && world.firms.get(*h).unwrap().archetype.is_some())
            .unwrap();
        cw_sim::economy::rivalry::note_grudge(
            &mut world,
            firm,
            other,
            5_000,
            cw_sim::economy::rivalry::Cause::Raid,
        );
        let bytes = postcard::to_allocvec(&world).unwrap();
        let back: World = postcard::from_bytes(&bytes).unwrap();
        for week in [4u32, 5, 6, 7] {
            let mut a = world.clone();
            let mut b = back.clone();
            a.tick = cw_core::calendar::Tick(week);
            b.tick = cw_core::calendar::Tick(week);
            let pa: Vec<String> = propose(&a, &dataset)
                .iter()
                .map(|p| format!("{:?}", p.intent))
                .collect();
            let pb: Vec<String> = propose(&b, &dataset)
                .iter()
                .map(|p| format!("{:?}", p.intent))
                .collect();
            assert_eq!(pa, pb);
        }
        assert_eq!(back.rivalry, world.rivalry);
    }
}
