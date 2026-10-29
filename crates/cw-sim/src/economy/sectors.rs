//! Sector divisions, A House Divided style: a firm enters a sector type in a region, then builds
//! plants inside it.
//!
//! A division is a firm's plants of one sector type in one region. It is derived from the
//! plants, never stored apart from them, so it cannot drift. Every plant in a division runs the
//! division's operating strategy; retooling moves the whole division. Opening a division pays an
//! entry fee and its founding plant is discounted. Building into a cell the firm already
//! dominates costs more: market share is the value of a firm's capacity against all capacity in
//! the cell plus the imports local plants are not meeting.

use cw_core::error::DomainError;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;

use crate::economy::operations::industry_index;
use crate::state::{FacilityStatus, RaidRecord, World};

/// The most blocks one build order may add. Past `FAST_BLOCKS` in flight, cost and lead time rise.
pub const MAX_BLOCKS_PER_ORDER: u8 = u8::MAX;
/// Blocks a firm can have under construction at once before building gets dearer and slower.
pub const FAST_BLOCKS: i64 = 40;

/// Paid once to the outside legal-services sector when a firm opens a division.
pub const ENTRY_FEE_CENTS: i64 = 10_000_000;
/// The founding plant of a division is built at half its strategy's capex per block.
pub const FOUNDING_BPS: i64 = 5_000;
/// Above this local share, new capacity costs more.
pub const DOMINANCE_THRESHOLD_BPS: i64 = 5_000;
/// Build-cost multiplier at 100 percent share with four or more rivals.
pub const DOMINANCE_MAX_BPS: i64 = 30_000;

/// Inputs saved by a plant of the state's primary and secondary specialized sector.
pub const SPECIALIZED_PRIMARY_BPS: i32 = 9_700;
pub const SPECIALIZED_SECONDARY_BPS: i32 = 9_850;
/// Extra inputs for a plant managed from another region of the home state, and from another
/// state. Plants in the home region are the calibration baseline.
pub const AWAY_IN_STATE_BPS: i32 = 10_250;
pub const AWAY_OUT_OF_STATE_BPS: i32 = 10_500;

/// The engine index of a strategy's sector type.
pub fn recipe_industry(dataset: &Dataset, recipe: u16) -> u8 {
    industry_index(&dataset.recipes[recipe as usize].industry).unwrap_or(0)
}

/// A firm's plants in one region and sector type, in handle order.
pub fn division_plants(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
) -> Vec<Handle> {
    let Some(entity) = world.firms.get(firm) else {
        return Vec::new();
    };
    let mut plants: Vec<Handle> = entity
        .facilities
        .iter()
        .copied()
        .filter(|handle| {
            world.facilities.get(*handle).is_some_and(|plant| {
                plant.region == region && recipe_industry(dataset, plant.recipe) == industry
            })
        })
        .collect();
    plants.sort();
    plants
}

/// The division's operating strategy: the strategy of its plants. None when the firm has no
/// plant of this sector type in the region.
pub fn division_strategy(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
) -> Option<u16> {
    division_plants(world, dataset, firm, region, industry)
        .first()
        .and_then(|handle| world.facilities.get(*handle))
        .map(|plant| plant.recipe)
}

/// One division, for reports and the interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Division {
    pub firm: Handle,
    pub region: u8,
    pub industry: u8,
    pub strategy: u16,
    pub plants: Vec<Handle>,
    pub blocks: u32,
}

/// Every division of a firm, ordered by region then sector type.
pub fn divisions_of(world: &World, dataset: &Dataset, firm: Handle) -> Vec<Division> {
    let Some(entity) = world.firms.get(firm) else {
        return Vec::new();
    };
    let mut keys: Vec<(u8, u8)> = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .map(|plant| (plant.region, recipe_industry(dataset, plant.recipe)))
        .collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|(region, industry)| {
            let plants = division_plants(world, dataset, firm, region, industry);
            let blocks = plants
                .iter()
                .filter_map(|handle| world.facilities.get(*handle))
                .map(|plant| plant.blocks as u32)
                .sum();
            let strategy = world
                .facilities
                .get(plants[0])
                .map(|p| p.recipe)
                .unwrap_or(0);
            Division {
                firm,
                region,
                industry,
                strategy,
                plants,
                blocks,
            }
        })
        .collect()
}

/// Weekly output value at reference price of a plant's rated capacity, counting plants under
/// construction: they are the capacity the cell will have.
pub fn capacity_value(dataset: &Dataset, recipe: u16, blocks: u8) -> i64 {
    let template = &dataset.recipes[recipe as usize];
    template.output_value(|id| {
        dataset
            .product_index
            .get(id)
            .map(|code| dataset.reference_price(*code))
            .unwrap_or(0)
    }) * blocks as i64
}

/// Capacity value by firm in one region and sector type, plus the unmet import value.
pub fn cell(
    world: &World,
    dataset: &Dataset,
    region: u8,
    industry: u8,
) -> (Vec<(Handle, i64)>, i64) {
    let mut by_firm: std::collections::BTreeMap<Handle, i64> = std::collections::BTreeMap::new();
    for (_, plant) in world.facilities.iter() {
        if plant.region != region || recipe_industry(dataset, plant.recipe) != industry {
            continue;
        }
        if world
            .firms
            .get(plant.firm)
            .is_none_or(|firm| !firm.operative())
        {
            continue;
        }
        if matches!(plant.status, FacilityStatus::Mothballed) {
            continue;
        }
        *by_firm.entry(plant.firm).or_default() +=
            capacity_value(dataset, plant.recipe, plant.blocks);
    }
    // Headroom: imports of the sector's goods into the region that no local plant is meeting.
    let outputs: Vec<u16> = dataset
        .recipes
        .iter()
        .filter(|recipe| industry_index(&recipe.industry) == Some(industry))
        .flat_map(|recipe| {
            recipe
                .outputs()
                .filter_map(|(id, _)| dataset.product_index.get(id).copied())
        })
        .collect();
    let headroom = world
        .gateways
        .iter()
        .filter(|gateway| gateway.region == region && outputs.contains(&gateway.product))
        .map(|gateway| gateway.smoothed_import_lots * dataset.reference_price(gateway.product))
        .sum();
    (by_firm.into_iter().collect(), headroom)
}

/// A firm's share of a cell in basis points, optionally after adding new capacity value.
pub fn share_bps(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
    added_value: i64,
) -> i64 {
    let (firms, headroom) = cell(world, dataset, region, industry);
    share_of(&firms, headroom, firm, added_value)
}

fn share_of(firms: &[(Handle, i64)], headroom: i64, firm: Handle, added_value: i64) -> i64 {
    let own = firms
        .iter()
        .find(|(handle, _)| *handle == firm)
        .map(|(_, value)| *value)
        .unwrap_or(0)
        + added_value;
    let total = firms.iter().map(|(_, value)| *value).sum::<i64>() + added_value + headroom;
    if total <= 0 {
        return 0;
    }
    (own as i128 * 10_000 / total as i128) as i64
}

/// Shares for many firms against one unchanged world: each cell is read once, however many
/// divisions in it ask. Only valid while the world it read is not changed.
#[derive(Default)]
pub struct ShareCache {
    cells: std::collections::BTreeMap<(u8, u8), Cell>,
}

/// Each firm's capacity value in a cell, and the cell's import headroom.
type Cell = (Vec<(Handle, i64)>, i64);

impl ShareCache {
    /// `share_bps` with no added capacity.
    pub fn share_bps(
        &mut self,
        world: &World,
        dataset: &Dataset,
        firm: Handle,
        region: u8,
        industry: u8,
    ) -> i64 {
        let (firms, headroom) = self
            .cells
            .entry((region, industry))
            .or_insert_with(|| cell(world, dataset, region, industry));
        share_of(firms, *headroom, firm, 0)
    }
}

/// Build-cost multiplier in basis points for new capacity in a cell. Up to the threshold share
/// it is 1; above it the extra cost rises with the square of the excess, to three times at 100
/// percent share. A cell with no rivals pays 35 percent of the extra, rising to all of it at
/// four or more rivals: dominance is a barrier to expansion, not a margin penalty.
pub fn dominance_bps(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
    added_value: i64,
) -> i64 {
    let share = share_bps(world, dataset, firm, region, industry, added_value);
    let (firms, _) = cell(world, dataset, region, industry);
    let rivals = firms.iter().filter(|(handle, _)| *handle != firm).count();
    dominance_multiplier_bps(share, rivals)
}

/// The dominance multiplier for a share in basis points and a count of rival firms in the cell.
pub fn dominance_multiplier_bps(share_bps: i64, rivals: usize) -> i64 {
    if share_bps <= DOMINANCE_THRESHOLD_BPS {
        return 10_000;
    }
    let rival_scale = 3_500 + 6_500 * rivals.min(4) as i64 / 4;
    let excess = (share_bps.min(10_000) - DOMINANCE_THRESHOLD_BPS) * 10_000
        / (10_000 - DOMINANCE_THRESHOLD_BPS);
    let ramp = excess * excess / 10_000;
    10_000 + (DOMINANCE_MAX_BPS - 10_000) * ramp / 10_000 * rival_scale / 10_000
}

/// Blocks a firm can have under construction at once at the normal price and speed: forty, and
/// a tenth of the blocks it already runs. Past it the firm competes with itself for crews and
/// building materials, so cost and lead time rise smoothly instead of the order being refused.
pub fn construction_capacity_blocks(world: &World, firm: Handle) -> i64 {
    let running: i64 = world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .facilities
                .iter()
                .filter_map(|handle| world.facilities.get(*handle))
                .filter(|plant| plant.status != FacilityStatus::Building)
                .map(|plant| plant.blocks as i64)
                .sum()
        })
        .unwrap_or(0);
    (FAST_BLOCKS + running / 10).min(400)
}

/// Blocks the firm already has under construction.
pub fn blocks_in_construction(world: &World, firm: Handle) -> i64 {
    world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .facilities
                .iter()
                .filter_map(|handle| world.facilities.get(*handle))
                .filter(|plant| plant.status == FacilityStatus::Building)
                .map(|plant| plant.blocks as i64)
                .sum()
        })
        .unwrap_or(0)
}

/// Blocks over the firm's construction capacity if it orders `new_blocks` now, and the capacity.
pub fn construction_excess(world: &World, firm: Handle, new_blocks: u8) -> (i64, i64) {
    let capacity = construction_capacity_blocks(world, firm).max(1);
    let load = blocks_in_construction(world, firm) + new_blocks as i64;
    ((load - capacity).max(0), capacity)
}

/// Cost multiplier for building past capacity: fifty percent more for every capacity's worth of
/// excess blocks, at most two and a half times.
pub fn rush_bps(excess: i64, capacity: i64) -> i64 {
    10_000 + (5_000 * excess / capacity.max(1)).min(15_000)
}

/// Build weeks with materials and crews stretched: another full build time for every capacity's
/// worth of excess blocks, at most three times the base.
pub fn stretched_weeks(base_weeks: u32, excess: i64, capacity: i64) -> u32 {
    let extra = (base_weeks as i64 * excess / capacity.max(1)).min(base_weeks as i64 * 2);
    base_weeks + extra as u32
}

/// What a build costs and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildCost {
    /// Capex after the founding discount and dominance multiplier.
    pub capex_cents: i64,
    /// Entry fee, nonzero only when the build opens a division.
    pub entry_fee_cents: i64,
    pub opens_division: bool,
    pub dominance_bps: i64,
    pub share_after_bps: i64,
    /// Cost multiplier for ordering past construction capacity, basis points (10,000 is none).
    pub rush_bps: i64,
}

impl BuildCost {
    pub fn total_cents(&self) -> i64 {
        self.capex_cents + self.entry_fee_cents
    }
}

/// The cost of building `blocks` of `recipe` in `region` for `firm`.
pub fn build_cost(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    recipe: u16,
    blocks: u8,
) -> BuildCost {
    let industry = recipe_industry(dataset, recipe);
    let opens = division_plants(world, dataset, firm, region, industry).is_empty();
    let capex = dataset.recipes[recipe as usize].capex_cents;
    let mut base = capex * blocks as i64;
    if opens {
        base -= capex * (10_000 - FOUNDING_BPS) / 10_000;
    }
    let added = capacity_value(dataset, recipe, blocks);
    // Dominance prices expansion of a division the firm already runs; opening one never pays
    // it, or the first entrant into an empty market would be charged as a monopolist.
    let dominance = if opens {
        10_000
    } else {
        dominance_bps(world, dataset, firm, region, industry, added)
    };
    let (excess, capacity) = construction_excess(world, firm, blocks);
    let rush = rush_bps(excess, capacity);
    BuildCost {
        capex_cents: (base as i128 * dominance as i128 / 10_000 * rush as i128 / 10_000) as i64,
        rush_bps: rush,
        entry_fee_cents: if opens { ENTRY_FEE_CENTS } else { 0 },
        opens_division: opens,
        dominance_bps: dominance,
        share_after_bps: share_bps(world, dataset, firm, region, industry, added),
    }
}

/// Input factor for a plant of `industry` in `region` run by a firm headquartered in `home`, in
/// basis points of the calibrated input need: away from home costs more, a state's specialized
/// sectors cost less.
pub fn location_factor_bps(dataset: &Dataset, home: u8, region: u8, industry: &str) -> i32 {
    let plant_state = dataset.regions.get(region as usize).map(|r| r.state());
    let home_state = dataset.regions.get(home as usize).map(|r| r.state());
    let mut bps: i64 = if region == home {
        10_000
    } else if plant_state == home_state {
        AWAY_IN_STATE_BPS as i64
    } else {
        AWAY_OUT_OF_STATE_BPS as i64
    };
    if let Some(state) = dataset.state_of(region) {
        if state.specialization.first().map(String::as_str) == Some(industry) {
            bps = bps * SPECIALIZED_PRIMARY_BPS as i64 / 10_000;
        } else if state.specialization.get(1).map(String::as_str) == Some(industry) {
            bps = bps * SPECIALIZED_SECONDARY_BPS as i64 / 10_000;
        }
    }
    bps as i32
}

/// Input factor for an existing plant (see `location_factor_bps`).
pub fn location_input_bps(world: &World, dataset: &Dataset, facility: Handle) -> i32 {
    let Some(plant) = world.facilities.get(facility) else {
        return 10_000;
    };
    let Some(firm) = world.firms.get(plant.firm) else {
        return 10_000;
    };
    location_factor_bps(
        dataset,
        firm.region,
        plant.region,
        &dataset.recipes[plant.recipe as usize].industry,
    )
}

/// Share of the defender's blocks at stake: 1 percent at even strength, rising smoothly to the
/// whole division at a five-to-one advantage. The defender keeps no guaranteed plant; instead a
/// bigger stake is harder to win (see `RAID_STAKE_DRAG_BPS`).
pub const RAID_STAKE_MIN_BPS: i64 = 100;
pub const RAID_STAKE_MAX_BPS: i64 = 10_000;
/// How much of the base success chance a whole-division stake costs the raider, basis points.
/// The chance is scaled by one minus this share of the stake.
pub const RAID_STAKE_DRAG_BPS: i64 = 8_500;
/// A defender's floor strength: five percent of its division's quarterly output value.
pub const RAID_DEFENCE_FLOOR_BPS: i64 = 500;
/// The attacker buys captured capacity at half its book value.
pub const RAID_PRICE_BPS: i64 = 5_000;
/// A division cannot be raided until this many weeks after its first plant was started, which
/// covers the opening of a game and any division opened later. A new company has no standing
/// marketing and a tiny book, so without it the first rival in the cell wins by default.
pub const RAID_GRACE_WEEKS: u32 = 26;
const RAID_LOG: usize = 64;

/// The first week a division can be raided: its oldest plant's start plus the grace period.
pub fn raid_open_tick(
    world: &World,
    dataset: &Dataset,
    defender: Handle,
    region: u8,
    industry: u8,
) -> u32 {
    division_plants(world, dataset, defender, region, industry)
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .map(|plant| plant.started.0)
        .min()
        .unwrap_or(0)
        + RAID_GRACE_WEEKS
}

/// What a raid would risk, for previews and the AI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaidQuote {
    pub attack_cents: i64,
    pub defence_cents: i64,
    pub chance_bps: i32,
    pub blocks_at_stake: u8,
    pub price_cents: i64,
    /// Share of the defender's division at stake, basis points.
    pub stake_bps: i32,
    /// Legal and retaliation costs of raiding the same rival again soon, on top of the budget.
    pub surcharge_cents: i64,
}

fn division_blocks_and_book(world: &World, plants: &[Handle]) -> (i64, i64) {
    plants
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .fold((0, 0), |(blocks, book), plant| {
            (
                blocks + plant.blocks as i64,
                book + (plant.capex_cents - plant.accumulated_depreciation).max(0),
            )
        })
}

/// Price a raid by `attacker` on `defender`'s division of `industry` in `region`.
pub fn raid_quote(
    world: &World,
    dataset: &Dataset,
    attacker: Handle,
    defender: Handle,
    region: u8,
    industry: u8,
    budget_cents: i64,
) -> RaidQuote {
    let quarter = |firm: Handle| {
        world
            .firms
            .get(firm)
            .map(|f| f.policies.marketing_budget_cents.max(0) * 13)
            .unwrap_or(0)
    };
    let defending = division_plants(world, dataset, defender, region, industry);
    let (blocks, book) = division_blocks_and_book(world, &defending);
    let output_value: i64 = defending
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .map(|plant| capacity_value(dataset, plant.recipe, plant.blocks) * 13)
        .sum();
    let attack = budget_cents.max(0) + quarter(attacker);
    let defence = (quarter(defender) + output_value * RAID_DEFENCE_FLOOR_BPS / 10_000).max(1);
    let advantage_bps = attack as i128 * 10_000 / defence as i128;
    let stake_bps = RAID_STAKE_MIN_BPS
        + (RAID_STAKE_MAX_BPS - RAID_STAKE_MIN_BPS)
            * (advantage_bps as i64 - 10_000).clamp(0, 40_000)
            / 40_000;
    let at_stake = (blocks * stake_bps / 10_000).max(1).min(blocks).max(0);
    let price = if blocks > 0 {
        book * at_stake / blocks * RAID_PRICE_BPS / 10_000
    } else {
        0
    };
    let base_chance = attack as i128 * 10_000 / (attack as i128 + defence as i128);
    let taken_bps = if blocks > 0 {
        at_stake * 10_000 / blocks
    } else {
        0
    };
    let chance = (base_chance * (10_000 - taken_bps as i128 * RAID_STAKE_DRAG_BPS as i128 / 10_000)
        / 10_000) as i32;
    let surcharge =
        crate::economy::structure::raid_surcharge(world, attacker, defender, budget_cents);
    RaidQuote {
        attack_cents: attack,
        defence_cents: defence,
        chance_bps: chance.clamp(500, 9_500),
        blocks_at_stake: at_stake.clamp(0, u8::MAX as i64) as u8,
        price_cents: price,
        stake_bps: taken_bps.clamp(0, 10_000) as i32,
        surcharge_cents: surcharge,
    }
}

/// Resolve a market raid: the budget buys advertising whatever happens; on success the
/// attacker takes the blocks at stake into its own division, at its own strategy, and pays the
/// defender half their book value.
pub fn raid(
    world: &mut World,
    dataset: &Dataset,
    attacker: Handle,
    defender: Handle,
    region: u8,
    industry: u8,
    budget_cents: i64,
) -> Result<RaidRecord, DomainError> {
    use cw_core::error::{ArithmeticFault, DomainError as E};
    if budget_cents <= 0 {
        return Err(E::Arithmetic {
            detail: ArithmeticFault::OutOfRange,
        });
    }
    if attacker == defender || crate::economy::ownership::same_group(world, attacker, defender) {
        return Err(E::WrongPhase {
            expected: "rival_firm",
            actual: "own_group",
        });
    }
    let own = division_plants(world, dataset, attacker, region, industry);
    let Some(target_plant) = own.first().copied() else {
        return Err(E::WrongPhase {
            expected: "own_division",
            actual: "no_division",
        });
    };
    if world.tick.0 < raid_open_tick(world, dataset, defender, region, industry) {
        return Err(E::WrongPhase {
            expected: "established_division",
            actual: "opening_grace",
        });
    }
    let defending = division_plants(world, dataset, defender, region, industry);
    if division_blocks_and_book(world, &defending).0 < 1 {
        return Err(E::WrongPhase {
            expected: "rival_division",
            actual: "too_small",
        });
    }
    let quote = raid_quote(
        world,
        dataset,
        attacker,
        defender,
        region,
        industry,
        budget_cents,
    );
    let cash = world
        .firms
        .get(attacker)
        .map(|f| f.book.cash())
        .unwrap_or(0);
    let needed = budget_cents + quote.price_cents + quote.surcharge_cents;
    if cash < needed {
        return Err(E::InsufficientCash {
            entity: attacker.0,
            required: needed,
            available: cash,
        });
    }
    crate::economy::research::buy_advertising(
        world,
        dataset,
        attacker,
        budget_cents,
        "market raid",
    );
    crate::economy::structure::charge_raid_surcharge(world, attacker, quote.surcharge_cents);
    let mut rng = cw_core::rng::Rng::entity_stream(
        world.seed ^ ((world.tick.0 as u64) << 32) ^ ((region as u64) << 8) ^ industry as u64,
        cw_core::rng::TAG_RAID,
        attacker,
    );
    let success = quote.blocks_at_stake > 0 && rng.chance_bps(quote.chance_bps);
    let mut record = RaidRecord {
        tick: world.tick.0,
        attacker,
        defender,
        region,
        industry,
        budget_cents,
        chance_bps: quote.chance_bps,
        success,
        blocks: 0,
        price_cents: 0,
    };
    if success {
        let (taken, capex, depreciation) =
            take_blocks(world, defender, &defending, quote.blocks_at_stake);
        let book = (capex - depreciation).max(0);
        let price = quote.price_cents.min(book);
        if let Some(entity) = world.firms.get_mut(defender) {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, price, "raid compensation")
                    .debit(Account::AccumulatedDepreciation, depreciation, "raid loss")
                    .debit(Account::OperatingExpense, book - price, "raid loss")
                    .credit(Account::FixedAssets, capex, "raid loss"),
            );
        }
        if let Some(entity) = world.firms.get_mut(attacker) {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, price, "raid acquisition")
                    .debit(Account::FixedAssets, price, "raid acquisition"),
            );
        }
        if let Some(plant) = world.facilities.get_mut(target_plant) {
            plant.blocks = plant.blocks.saturating_add(taken);
            plant.capex_cents += price;
        }
        record.blocks = taken;
        record.price_cents = price;
    }
    crate::economy::structure::note_raid(world, &record, quote.stake_bps);
    world.raids.push(record.clone());
    if world.raids.len() > RAID_LOG {
        world.raids.remove(0);
    }
    Ok(record)
}

/// Remove up to `wanted` blocks from a defender's plants, largest first, never its last block.
/// Returns the blocks taken and their capex and accumulated depreciation.
fn take_blocks(
    world: &mut World,
    defender: Handle,
    plants: &[Handle],
    wanted: u8,
) -> (u8, i64, i64) {
    let mut order: Vec<(u8, Handle)> = plants
        .iter()
        .filter_map(|handle| world.facilities.get(*handle).map(|p| (p.blocks, *handle)))
        .collect();
    order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let total: i64 = order.iter().map(|(blocks, _)| *blocks as i64).sum();
    let mut remaining = (wanted as i64).min(total).max(0);
    let (mut taken, mut capex, mut depreciation) = (0u8, 0i64, 0i64);
    for (_, handle) in order {
        if remaining == 0 {
            break;
        }
        let Some(plant) = world.facilities.get_mut(handle) else {
            continue;
        };
        let take = (plant.blocks as i64).min(remaining);
        let share_capex = plant.capex_cents * take / plant.blocks as i64;
        let share_dep = plant.accumulated_depreciation * take / plant.blocks as i64;
        let ratio = (plant.blocks as i64 - take, plant.blocks as i64);
        for tier in 0..3 {
            plant.workers[tier] = plant.workers[tier] * ratio.0 / ratio.1;
        }
        plant.blocks -= take as u8;
        plant.capex_cents -= share_capex;
        plant.accumulated_depreciation -= share_dep;
        let emptied = plant.blocks == 0;
        taken += take as u8;
        capex += share_capex;
        depreciation += share_dep;
        remaining -= take;
        if emptied {
            world.facilities.remove(handle);
            if let Some(firm) = world.firms.get_mut(defender) {
                firm.facilities.retain(|h| *h != handle);
            }
        }
    }
    (taken, capex, depreciation)
}
