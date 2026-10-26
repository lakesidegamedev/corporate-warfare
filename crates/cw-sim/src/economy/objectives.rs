//! Board objectives: each quarter the board of the player's primary company sets one goal
//! tied to the corporation game (a new region, a new state, a market share, an operating
//! profit). Meeting it by the quarter's end pays the player a management bonus from the
//! company: two percent of its quarter's operating profit, at least $25,000, never more than
//! its free cash. The bonus moves money; it never mints it.

use std::collections::BTreeSet;

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::economy::sectors;
use crate::state::World;

pub const QUARTER_WEEKS: u32 = 13;
pub const MIN_BONUS_CENTS: i64 = 2_500_000;
pub const BONUS_BPS: i64 = 200;
const HISTORY: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectiveKind {
    /// Operate in more regions than at the start of the quarter.
    NewRegion { baseline: u32 },
    /// Average weekly operating profit over the last four weeks at or above the target.
    WeeklyEbit { target_cents: i64 },
    /// Reach a share of the home division's market.
    MarketShare {
        region: u8,
        industry: u8,
        target_bps: i64,
    },
    /// Operate in more states than at the start of the quarter.
    NewState { baseline: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    pub firm: Handle,
    pub set: u32,
    pub due: u32,
    pub kind: ObjectiveKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectiveResult {
    pub objective: Objective,
    pub met: bool,
    pub closed: u32,
    pub bonus_cents: i64,
}

/// Regions and states the firm operates divisions in.
fn reach(world: &World, dataset: &Dataset, firm: Handle) -> (u32, u32) {
    let mut regions = BTreeSet::new();
    let mut states = BTreeSet::new();
    for division in sectors::divisions_of(world, dataset, firm) {
        regions.insert(division.region);
        states.insert(dataset.regions[division.region as usize].state().to_owned());
    }
    (regions.len() as u32, states.len() as u32)
}

fn recent_ebit(world: &World, firm: Handle, weeks: usize) -> i64 {
    world
        .firms
        .get(firm)
        .map(|f| {
            let closes = f.book.recent_weeks();
            let take = closes.len().min(weeks);
            closes[closes.len() - take..].iter().map(|w| w.ebit()).sum()
        })
        .unwrap_or(0)
}

/// How far along the objective is, in basis points of its target.
pub fn progress_bps(world: &World, dataset: &Dataset, objective: &Objective) -> i64 {
    let firm = objective.firm;
    match &objective.kind {
        ObjectiveKind::NewRegion { baseline } => {
            if reach(world, dataset, firm).0 > *baseline {
                10_000
            } else {
                0
            }
        }
        ObjectiveKind::NewState { baseline } => {
            if reach(world, dataset, firm).1 > *baseline {
                10_000
            } else {
                0
            }
        }
        ObjectiveKind::WeeklyEbit { target_cents } => {
            let average = recent_ebit(world, firm, 4) / 4;
            if *target_cents <= 0 {
                10_000
            } else {
                (average.max(0) * 10_000 / target_cents).min(10_000)
            }
        }
        ObjectiveKind::MarketShare {
            region,
            industry,
            target_bps,
        } => {
            let share = sectors::share_bps(world, dataset, firm, *region, *industry, 0);
            (share * 10_000 / (*target_bps).max(1)).min(10_000)
        }
    }
}

/// Set a new objective at the start of each quarter and close the current one when it is met
/// or due.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    if let Some(objective) = world.objective.clone() {
        let met = progress_bps(world, dataset, &objective) >= 10_000;
        if met || world.tick.0 >= objective.due {
            let bonus = if met {
                pay_bonus(world, objective.firm)
            } else {
                0
            };
            world.objective_history.push(ObjectiveResult {
                objective,
                met,
                closed: world.tick.0,
                bonus_cents: bonus,
            });
            if world.objective_history.len() > HISTORY {
                world.objective_history.remove(0);
            }
            world.objective = None;
        }
    }
    if world.objective.is_some() || world.tick.0 % QUARTER_WEEKS != 1 {
        return;
    }
    let Some(firm) = world.investor.primary_group else {
        return;
    };
    if world.firms.get(firm).is_none_or(|f| !f.operative()) {
        return;
    }
    let quarter = world.tick.0 / QUARTER_WEEKS;
    let (regions, states) = reach(world, dataset, firm);
    let home = sectors::divisions_of(world, dataset, firm)
        .into_iter()
        .max_by_key(|d| (d.blocks, std::cmp::Reverse(d.region)));
    let average = recent_ebit(world, firm, 4) / 4;
    let kind = match quarter % 4 {
        0 => ObjectiveKind::NewRegion { baseline: regions },
        1 => ObjectiveKind::WeeklyEbit {
            target_cents: (average + average.abs() / 4).max(average + 500_000),
        },
        2 => match home {
            Some(division) => {
                let share =
                    sectors::share_bps(world, dataset, firm, division.region, division.industry, 0);
                ObjectiveKind::MarketShare {
                    region: division.region,
                    industry: division.industry,
                    target_bps: (share + 1_000).min(9_500),
                }
            }
            None => ObjectiveKind::NewRegion { baseline: regions },
        },
        _ => ObjectiveKind::NewState { baseline: states },
    };
    world.objective = Some(Objective {
        firm,
        set: world.tick.0,
        due: world.tick.0 + QUARTER_WEEKS,
        kind,
    });
}

/// Pay the management bonus from the company to the investor.
fn pay_bonus(world: &mut World, firm: Handle) -> i64 {
    let quarter = recent_ebit(world, firm, QUARTER_WEEKS as usize);
    let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
    let bonus = (quarter.max(0) * BONUS_BPS / 10_000)
        .max(MIN_BONUS_CENTS)
        .min(cash.max(0));
    if bonus <= 0 {
        return 0;
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, bonus, "management bonus")
                .debit(Account::OperatingExpense, bonus, "management bonus"),
        );
    }
    world.investor.cash_cents += bonus;
    let _ = world.investor.book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, bonus, "management bonus")
            .credit(Account::OtherIncome, bonus, "management bonus"),
    );
    bonus
}
