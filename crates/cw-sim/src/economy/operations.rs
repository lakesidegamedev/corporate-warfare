//! Corporate operations after A House Divided: sector focus, state conditions, logistics
//! strength and sprawl, expressed as physical efficiency and real overhead payments.
//!
//! - Sector focus. A firm's `industry` is its primary sector type; it may add one secondary
//!   type. Plants of the primary type use 5 percent fewer inputs, of the secondary type 2.5
//!   percent fewer. Any other plant pays a coordination overhead of 10 percent of its payroll.
//!   Changing focus costs 5 percent of every plant's payroll for four weeks and locks further
//!   changes for eight.
//! - State conditions. Power-grid reliability below 95 percent, low workforce skill (skilled
//!   sectors) and low broadband (digital sectors) cut output, up to 4 percent each; good skill
//!   and broadband save up to 4 percent of inputs instead, so no plant exceeds its rated
//!   capacity. Roads move freight cost, and crime costs retail and property plants a
//!   shrinkage share of their output value.
//! - Sprawl. Overhead grows smoothly with the number of divisions: 0.18 percent of weekly revenue
//!   at ten, 0.72 at twenty, 1.6 at thirty, doubled with a secondary type. Logistics strength
//!   (0..200), built by a weekly logistics budget, takes up to half of it off, and so does each
//!   logistics division the firm owns (six percent each, up to forty).
//!
//! Every overhead is paid in cash to the outside services sector, so no money is created or
//! destroyed. All arithmetic is integer basis points.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;

use crate::state::{FacilityStatus, INDUSTRY_NAMES, World};

pub const PRIMARY_INPUT_BPS: i32 = 9_500;
pub const SECONDARY_INPUT_BPS: i32 = 9_750;
pub const OFF_FOCUS_OVERHEAD_BPS: i64 = 1_000;
pub const FOCUS_TRANSITION_OVERHEAD_BPS: i64 = 500;
pub const FOCUS_TRANSITION_WEEKS: u32 = 4;
pub const FOCUS_COOLDOWN_WEEKS: u32 = 8;
pub const MAX_LOGISTICS_STRENGTH: i32 = 200;

const SKILLED: [&str; 5] = [
    "technology",
    "chemical_industries",
    "healthcare",
    "manufacturing",
    "defense",
];
const DIGITAL: [&str; 3] = ["technology", "telecommunications", "media"];
const SHRINKAGE: [&str; 2] = ["retail", "real_estate"];

/// How a plant's sector type relates to its owner's focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Primary,
    Secondary,
    Off,
}

/// Share of a plant's capex, in basis points, charged to bring it back from mothballs: rehiring,
/// recommissioning and restocking spares. Workers are hired again through the labour market.
pub const REOPEN_COST_BPS: i64 = 500;

/// Mothball a plant: it stops producing and releases its whole workforce, so it carries no
/// payroll, maintenance, rent or depreciation until it reopens.
pub fn mothball(facility: &mut crate::state::Facility, now: Tick) {
    facility.status = FacilityStatus::Mothballed;
    facility.mothballed_since = Some(now);
    facility.workers = [0; 3];
}

/// Cash needed to reopen a mothballed plant.
pub fn reopen_cost(facility: &crate::state::Facility) -> i64 {
    (facility.capex_cents.max(0) * REOPEN_COST_BPS / 10_000).max(100_000)
}

pub fn industry_index(name: &str) -> Option<u8> {
    // Same positions as INDUSTRY_NAMES, as a match so the hot paths do not scan strings.
    Some(match name {
        "agriculture" => 0,
        "extraction" => 1,
        "energy" => 2,
        "chemical_industries" => 3,
        "manufacturing" => 4,
        "construction" => 5,
        "logistics" => 6,
        "retail" => 7,
        "healthcare" => 8,
        "financial" => 9,
        "technology" => 10,
        "media" => 11,
        "telecommunications" => 12,
        "real_estate" => 13,
        "defense" => 14,
        _ => return None,
    })
}

pub fn focus(world: &World, dataset: &Dataset, facility: Handle) -> Focus {
    let Some(plant) = world.facilities.get(facility) else {
        return Focus::Off;
    };
    let Some(firm) = world.firms.get(plant.firm) else {
        return Focus::Off;
    };
    let sector = industry_index(&dataset.recipes[plant.recipe as usize].industry);
    if sector == Some(firm.industry) {
        Focus::Primary
    } else if sector.is_some() && sector == firm.secondary_industry {
        Focus::Secondary
    } else {
        Focus::Off
    }
}

/// Input requirement in basis points of the recipe for this plant: sector focus, then any
/// favourable state conditions (skill, broadband), which save inputs rather than push output
/// beyond the plant's rated capacity.
pub fn input_bps(world: &World, dataset: &Dataset, facility: Handle) -> i32 {
    let focus_bps = match focus(world, dataset, facility) {
        Focus::Primary => PRIMARY_INPUT_BPS,
        Focus::Secondary => SECONDARY_INPUT_BPS,
        Focus::Off => 10_000,
    };
    let (_, bonus) = condition_terms(world, dataset, facility, false);
    let location = crate::economy::sectors::location_input_bps(world, dataset, facility) as i64;
    (focus_bps as i64 * (10_000 - bonus) as i64 / 10_000 * location / 10_000) as i32
}

/// Output multiplier from adverse state conditions, in basis points, never above 10000: a plant
/// cannot produce beyond its rated capacity.
pub fn output_bps(world: &World, dataset: &Dataset, facility: Handle, utility: bool) -> i32 {
    let (penalty, _) = condition_terms(world, dataset, facility, utility);
    10_000 - penalty
}

/// (output penalty, input saving) in basis points from the plant's state conditions: grid
/// reliability below 95 percent, and skill and broadband either side of their pivots.
pub fn condition_terms(
    world: &World,
    dataset: &Dataset,
    facility: Handle,
    utility: bool,
) -> (i32, i32) {
    let Some(plant) = world.facilities.get(facility) else {
        return (0, 0);
    };
    let Some(region) = dataset.regions.get(plant.region as usize) else {
        return (0, 0);
    };
    let conditions = region.conditions;
    let industry = dataset.recipes[plant.recipe as usize].industry.as_str();
    let mut delta = 0;
    if !utility {
        delta -= (9_500 - conditions.power_grid_bps).max(0) * 400 / 1_000;
    }
    if SKILLED.contains(&industry) {
        delta += ((conditions.skill_bps - 5_000) * 400 / 5_000).clamp(-400, 400);
    }
    if DIGITAL.contains(&industry) {
        delta += ((conditions.broadband_bps - 2_000) * 400 / 4_000).clamp(-400, 400);
    }
    if delta < 0 { (-delta, 0) } else { (0, delta) }
}

/// Freight cost multiplier for a lane from the two states' road condition, in basis points.
pub fn road_bps(dataset: &Dataset, from: u8, to: u8) -> i64 {
    let road = |index: u8| {
        dataset
            .regions
            .get(index as usize)
            .map(|region| region.conditions.road_bps)
            .unwrap_or(6_000) as i64
    };
    let average = (road(from) + road(to)) / 2;
    (10_000 + (6_000 - average) / 2).clamp(9_000, 11_000)
}

/// Weekly payroll of one plant at its owner's wage offers.
fn plant_payroll(world: &World, facility: Handle) -> i64 {
    let Some(plant) = world.facilities.get(facility) else {
        return 0;
    };
    let Some(firm) = world.firms.get(plant.firm) else {
        return 0;
    };
    (0..3)
        .map(|tier| plant.workers[tier] * firm.policies.wage_offer[tier])
        .sum()
}

/// Sprawl overhead in basis points of weekly revenue for a number of divisions. A smooth square
/// curve with no cliff: `0.18 * d * d`. Logistics strength takes off up to half of it and every
/// logistics division the firm owns takes off six percent, up to forty; both multiply.
pub fn sprawl_rate_bps(
    divisions: i64,
    logistics_strength: i32,
    owned_logistics: i64,
    has_secondary: bool,
) -> i64 {
    let d = divisions.max(0);
    let mut rate = d * d * 18 * 100; // hundredths of a basis point
    let strength = logistics_strength.clamp(0, MAX_LOGISTICS_STRENGTH) as i64;
    rate = rate * (400 - strength) / 400;
    rate = rate * (10_000 - (owned_logistics.max(0) * 600).min(4_000)) / 10_000;
    if has_secondary {
        rate *= 2;
    }
    rate / 10_000
}

/// The division count at which sprawl costs one percent of weekly revenue, for display.
pub fn sprawl_onset(logistics_strength: i32, owned_logistics: i64, has_secondary: bool) -> i64 {
    (1..=200)
        .find(|d| sprawl_rate_bps(*d, logistics_strength, owned_logistics, has_secondary) >= 100)
        .unwrap_or(200)
}

/// Divisions of a firm in the logistics sector.
pub fn owned_logistics_divisions(world: &World, dataset: &Dataset, firm: Handle) -> i64 {
    let logistics = industry_index("logistics").unwrap_or(0);
    crate::economy::sectors::divisions_of(world, dataset, firm)
        .iter()
        .filter(|d| d.industry == logistics)
        .count() as i64
}

/// Logistics strength gained in a week from the budget actually paid.
pub fn logistics_growth(paid_cents: i64) -> i32 {
    match paid_cents {
        p if p >= 10_000_000 => 3,
        p if p >= 2_500_000 => 2,
        p if p >= 500_000 => 1,
        _ => 0,
    }
}

fn pay_outside(world: &mut World, firm: Handle, amount: i64, memo: &str) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let cash = world.firm(firm).map(|f| f.book.cash().max(0)).unwrap_or(0);
    let paid = amount.min(cash);
    if paid <= 0 {
        return 0;
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new().credit(Account::Cash, paid, memo).debit(
                Account::OperatingExpense,
                paid,
                memo,
            ),
        );
    }
    let _ = world.outside.gateway_book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, paid, memo)
            .credit(Account::Revenue, paid, memo),
    );
    paid
}

/// Weekly operations charges and logistics strength, after production and payroll.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for firm in handles {
        let Some(entity) = world.firms.get(firm) else {
            continue;
        };
        if !entity.operative() {
            continue;
        }
        let plants: Vec<Handle> = entity
            .facilities
            .iter()
            .copied()
            .filter(|handle| {
                world
                    .facilities
                    .get(*handle)
                    .map(|plant| plant.status != FacilityStatus::Building)
                    .unwrap_or(false)
            })
            .collect();
        let in_transition = world.tick.0 < entity.focus_transition_until;
        let has_secondary = entity.secondary_industry.is_some();
        let strength = entity.logistics_strength;
        let budget = entity.policies.logistics_budget_cents;
        let revenue = entity
            .book
            .recent_weeks()
            .last()
            .map(|week| week.revenue)
            .unwrap_or(0)
            .max(0);

        let mut overhead = 0i64;
        let mut shrinkage = 0i64;
        for plant in &plants {
            let payroll = plant_payroll(world, *plant);
            if focus(world, dataset, *plant) == Focus::Off {
                overhead += payroll * OFF_FOCUS_OVERHEAD_BPS / 10_000;
            }
            if in_transition {
                overhead += payroll * FOCUS_TRANSITION_OVERHEAD_BPS / 10_000;
            }
            let Some(facility) = world.facilities.get(*plant) else {
                continue;
            };
            let recipe = &dataset.recipes[facility.recipe as usize];
            if SHRINKAGE.contains(&recipe.industry.as_str()) {
                let crime = dataset.regions[facility.region as usize]
                    .conditions
                    .crime_bps as i64;
                let rate = (crime - 7_000).max(0) / 100; // bps of output value
                let value = facility.last_output.lots()
                    * dataset
                        .product_index
                        .get(&recipe.output)
                        .map(|code| dataset.reference_price(*code))
                        .unwrap_or(0);
                shrinkage += value * rate / 10_000;
            }
        }
        // Sprawl counts divisions (sector types run in a region), as in A House Divided's
        // per-sector rule: adding plants to an existing division never sprawls.
        let divisions = crate::economy::sectors::divisions_of(world, dataset, firm).len() as i64;
        let owned = owned_logistics_divisions(world, dataset, firm);
        let sprawl = revenue * sprawl_rate_bps(divisions, strength, owned, has_secondary) / 10_000;

        pay_outside(world, firm, overhead, "coordination overhead");
        pay_outside(world, firm, shrinkage, "shrinkage");
        pay_outside(world, firm, sprawl, "sprawl overhead");
        let paid = pay_outside(world, firm, budget, "logistics programme");
        if let Some(entity) = world.firms.get_mut(firm) {
            let growth = logistics_growth(paid);
            if growth > 0 {
                entity.logistics_strength =
                    (entity.logistics_strength + growth).min(MAX_LOGISTICS_STRENGTH);
            } else if world.tick.0.is_multiple_of(13) {
                entity.logistics_strength = (entity.logistics_strength - 1).max(0);
            }
        }
    }
}

/// Change a firm's sector focus: a four-week transition overhead, then an eight-week lock.
pub fn set_focus(
    world: &mut World,
    firm: Handle,
    primary: u8,
    secondary: Option<u8>,
) -> cw_core::error::DomainResult<()> {
    use cw_core::error::{ArithmeticFault, DomainError, EntityKind};
    if primary as usize >= INDUSTRY_NAMES.len()
        || secondary
            .map(|s| s as usize >= INDUSTRY_NAMES.len() || s == primary)
            .unwrap_or(false)
    {
        return Err(DomainError::Arithmetic {
            detail: ArithmeticFault::OutOfRange,
        });
    }
    let now = world.tick.0;
    let Some(entity) = world.firms.get_mut(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    if now < entity.focus_cooldown_until {
        return Err(DomainError::WrongPhase {
            expected: "focus_cooldown_elapsed",
            actual: "focus_cooldown",
        });
    }
    if entity.industry == primary && entity.secondary_industry == secondary {
        return Ok(());
    }
    entity.industry = primary;
    entity.secondary_industry = secondary;
    entity.focus_transition_until = now + FOCUS_TRANSITION_WEEKS;
    entity.focus_cooldown_until = now + FOCUS_COOLDOWN_WEEKS;
    let _ = Tick(now);
    Ok(())
}

#[cfg(test)]
mod industry_index_tests {
    #[test]
    fn matches_the_name_table() {
        for (i, name) in super::INDUSTRY_NAMES.iter().enumerate() {
            assert_eq!(super::industry_index(name), Some(i as u8));
        }
        assert_eq!(super::industry_index("nonsense"), None);
    }
}
