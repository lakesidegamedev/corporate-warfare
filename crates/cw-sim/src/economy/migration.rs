//! Workforce migration (1.9): workers move between regions on the gap in pay and open jobs, and
//! their households move with them.
//!
//! Each tier of each region has a pull: its wage against where it started, plus its open jobs
//! (positions a named employer cannot fill, as a share of the workforce), less a crowding term
//! that grows with the region's population against where it started. The background sector
//! absorbs everyone not in a named job, so open jobs stand in for unemployment here. Where one
//! region pulls more than another by over three points, workers move toward it at up to four
//! percent a year of the people free to move (those not on a named payroll), less with
//! distance (halved at 400 km), halved again across a state line and cut a sixth when the
//! destination is a right to work state. Regions on different currencies do not exchange
//! workers. The flow goes through the existing labour market: it changes who lives where, and
//! `labour::clear` then hires from the new pool.
//!
//! Households move in proportion to the people: the same share of the source region's savings,
//! baseline income and baseline demand moves to the destination, so demand follows the workers
//! and nothing is created or lost (all moved amounts are integer shares of the source).
//!
//! State is saved with the rival plans (`World.plans.migration`, trailer `PLAN`): the start
//! references, the milli-worker carries so rounding never drifts, and each region's net movers
//! by week for the region dossier.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::state::World;
use cw_data::Dataset;

/// The pull gap, in basis points, below which nobody moves.
pub const THRESHOLD_BPS: i64 = 300;
/// Most a source loses to one destination in a year, as basis points of the people free to move.
pub const MAX_ANNUAL_BPS: i64 = 400;
/// Distance at which the flow is halved.
pub const HALF_KM: i64 = 400;
/// Share of the flow that survives a state line, in basis points.
pub const BORDER_BPS: i64 = 5_000;
/// Share that survives into a right to work state.
pub const RIGHT_TO_WORK_BPS: i64 = 8_300;
/// Largest weight open jobs and crowding carry in the pull, in basis points.
const CAP_BPS: i64 = 1_500;
const NET_KEPT: usize = 104;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationState {
    /// Wage and population per region and tier when the first pass ran.
    pub reference_wage: Vec<[i64; 3]>,
    pub reference_pop: Vec<[i64; 3]>,
    /// Milli-workers owed on each (from, to, tier) flow.
    pub carry: BTreeMap<(u8, u8, u8), i64>,
    /// Net movers into each region by week (negative is a loss), newest last.
    pub net: Vec<Vec<i32>>,
    /// Net movers into each region since the start.
    pub total: Vec<i64>,
}

impl MigrationState {
    /// Net movers into `region` over the last `weeks`.
    pub fn net_over(&self, region: usize, weeks: usize) -> i64 {
        self.net
            .get(region)
            .map(|v| v.iter().rev().take(weeks).map(|n| *n as i64).sum())
            .unwrap_or(0)
    }
}

/// The pull of a tier in a region, basis points around 10,000.
fn pull(
    world: &World,
    state: &MigrationState,
    open: &[[i64; 3]],
    region: usize,
    tier: usize,
) -> i64 {
    let r = &world.regions[region];
    let lab = &r.labour[tier];
    let ref_wage = state.reference_wage[region][tier].max(1);
    let ref_pop = state.reference_pop[region][tier].max(1);
    let wage = lab.prevailing_wage_cents.max(1) * 10_000 / ref_wage;
    let cap = (lab.population * 9_500 / 10_000).max(1);
    let vacancy = (open[region][tier] * 10_000 / cap).clamp(0, CAP_BPS);
    let crowd = ((lab.population - ref_pop) * 10_000 / ref_pop / 2).clamp(-CAP_BPS, CAP_BPS);
    wage + vacancy - crowd
}

/// Friction on a flow from `a` to `b`, basis points of the unobstructed flow.
pub fn friction_bps(world: &World, dataset: &Dataset, a: usize, b: usize) -> i64 {
    let km = dataset.km(a as u8, b as u8) as i64;
    let mut f = 10_000 * HALF_KM / (HALF_KM + km);
    let (sa, sb) = (
        world.civics.state_of(a as u8),
        world.civics.state_of(b as u8),
    );
    if sa != sb {
        f = f * BORDER_BPS / 10_000;
    }
    if let Some(s) = sb
        && world
            .civics
            .states
            .get(s)
            .is_some_and(|st| st.laws.right_to_work)
        && sa != sb
    {
        f = f * RIGHT_TO_WORK_BPS / 10_000;
    }
    f
}

/// Weekly: move workers and their households. Runs before the labour market clears.
#[allow(clippy::needless_range_loop)]
pub fn weekly(world: &mut World, dataset: &Dataset) {
    let n = world.regions.len();
    if n < 2 {
        return;
    }
    {
        let m = &mut world.plans.migration;
        if m.reference_wage.len() != n {
            m.reference_wage = world
                .regions
                .iter()
                .map(|r| std::array::from_fn(|t| r.labour[t].prevailing_wage_cents.max(1)))
                .collect();
            m.reference_pop = world
                .regions
                .iter()
                .map(|r| std::array::from_fn(|t| r.labour[t].population.max(1)))
                .collect();
            m.net = vec![Vec::new(); n];
            m.total = vec![0; n];
        }
    }
    let open = crate::economy::labour::open_jobs(world, dataset);
    let named = crate::economy::labour::named_workers(world);
    let state = world.plans.migration.clone();
    let currency: Vec<_> = (0..n)
        .map(|r| crate::economy::fx::region_currency(dataset, r as u8))
        .collect();
    let before_pop: Vec<i64> = world
        .regions
        .iter()
        .map(|r| r.labour.iter().map(|l| l.population).sum())
        .collect();
    let mut free: Vec<[i64; 3]> = (0..n)
        .map(|r| {
            std::array::from_fn(|t| {
                let cap = world.regions[r].labour[t].population * 9_500 / 10_000;
                (cap - named[r][t]).max(0)
            })
        })
        .collect();
    let mut carry = state.carry.clone();
    // (from, to) -> movers across tiers.
    let mut pairs: BTreeMap<(usize, usize), i64> = BTreeMap::new();
    let mut moves: Vec<(usize, usize, usize, i64)> = Vec::new();
    for tier in 0..3 {
        let pulls: Vec<i64> = (0..n)
            .map(|r| pull(world, &state, &open, r, tier))
            .collect();
        for a in 0..n {
            for b in 0..n {
                if a == b || currency[a] != currency[b] {
                    continue;
                }
                let gap = pulls[b] - pulls[a];
                if gap <= THRESHOLD_BPS {
                    continue;
                }
                let annual = ((gap - THRESHOLD_BPS) / 4).min(MAX_ANNUAL_BPS);
                let f = friction_bps(world, dataset, a, b);
                let key = (a as u8, b as u8, tier as u8);
                // Milli-workers a week: free people x annual rate x friction / 52.
                let milli = free[a][tier] as i128 * 1_000 * annual as i128 * f as i128
                    / 10_000
                    / 10_000
                    / 52;
                let owed = carry.get(&key).copied().unwrap_or(0) + milli as i64;
                let whole = (owed / 1_000).min(free[a][tier]);
                carry.insert(key, owed - whole * 1_000);
                if whole > 0 {
                    free[a][tier] -= whole;
                    moves.push((a, b, tier, whole));
                    *pairs.entry((a, b)).or_default() += whole;
                }
            }
        }
    }
    carry.retain(|_, v| *v != 0);
    // Households move in proportion to the people.
    for ((a, b), moved) in &pairs {
        let people = before_pop[*a].max(1);
        let share = |amount: i64| (amount as i128 * *moved as i128 / people as i128) as i64;
        let (savings, income, lots): (i64, i64, Vec<(u16, i64)>) = {
            let h = &world.regions[*a].households;
            (
                share(h.savings_cents.max(0)),
                share(h.baseline_income_cents.max(0)),
                h.baseline_lots
                    .iter()
                    .map(|(p, l)| (*p, share(*l)))
                    .collect(),
            )
        };
        {
            let h = &mut world.regions[*a].households;
            h.savings_cents -= savings;
            h.baseline_income_cents -= income;
            for (p, l) in &lots {
                if let Some(v) = h.baseline_lots.get_mut(p) {
                    *v -= l;
                }
            }
        }
        let h = &mut world.regions[*b].households;
        h.savings_cents += savings;
        h.baseline_income_cents += income;
        for (p, l) in lots {
            *h.baseline_lots.entry(p).or_default() += l;
        }
    }
    let mut net = vec![0i64; n];
    for (a, b, tier, count) in moves {
        world.regions[a].labour[tier].population -= count;
        world.regions[b].labour[tier].population += count;
        net[a] -= count;
        net[b] += count;
    }
    let m = &mut world.plans.migration;
    m.carry = carry;
    for r in 0..n {
        let v = &mut m.net[r];
        v.push(net[r].clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        if v.len() > NET_KEPT {
            v.remove(0);
        }
        m.total[r] += net[r];
    }
}
