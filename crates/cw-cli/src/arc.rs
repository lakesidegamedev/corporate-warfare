//! `cw-cli arc`: measures whether a ten-year career is a contest.
//!
//! For each seed it plays one sandbox founder career with a reference policy, watches the world
//! every week through a read-only observer, and aggregates: the player's rank and net worth, the
//! league's lead changes, firm turnover, zombie firm-weeks, price ranges, deals, how much the
//! player changed the world (against a `none` run of the same seed) and speed. It changes no
//! rule: the observer only reads `World`.
//!
//! Definitions used (all stated again in the JSON `definitions` block):
//! * Size and rank use the engine's own valuation (`firm.valuation.equity_cents`), falling back
//!   to book equity before the first valuation. The league is every operative firm that no other
//!   firm controls; the player's other investor-controlled firms are folded into the player and
//!   are not rivals. The player's rank is its primary company's place in that league.
//! * Bust: the primary company entered restructuring, liquidation or dissolution.
//! * Hostile moves aimed at the player's group: market raids, hostile deals, activist campaigns
//!   and rival takeover plans (a hostile deal that belongs to a counted plan is not counted
//!   twice). Rival move log entries carry no target, so they are not counted.
//! * Zombie firm-week: operative firm with book cash at or below zero and no workers.
//!   Stressed firm-week: the `diagnose` rule, cash under four weeks of mandatory outflow or no
//!   revenue last week.
//! * Price range: per good and year, (max - min) of the weekly closing price over the reference
//!   price. Cap or floor: the closing price at or above the cheapest gateway sell price (what
//!   imports cost) or at or below the dearest gateway buy price (what exports earn).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::time::Instant;

use cw_core::ids::Handle;
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::economy::ownership::ultimate_controller;
use cw_sim::state::{DealState, FirmStatus, HolderId, World};
use serde::{Deserialize, Serialize};

use crate::policy::{PolicyRunner, ReferencePolicy};

const YEAR: u32 = 52;
/// A new leader must hold first place this many weeks to count as a change of hands.
const LEADER_HOLD_WEEKS: usize = 13;
const DEFAULT_GATES: &str = include_str!("../../../tools/arc_gates.json");

// ---------------------------------------------------------------------------------------------
// Pure metric maths (unit tested on small fixtures)
// ---------------------------------------------------------------------------------------------

/// Median of integers, lower middle averaged for even counts. Empty gives None.
pub fn median_i64(values: &[i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let middle = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2
    } else {
        sorted[middle]
    })
}

pub fn median_f64(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    let middle = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    })
}

/// The value at fraction `num/den` of the sorted list (nearest rank, rounded down).
pub fn quantile_i64(values: &[i64], num: usize, den: usize) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Some(sorted[(sorted.len() - 1) * num / den])
}

/// Times first place changed hands. `leaders` is the leader's id each week. A challenger takes
/// the lead only after leading `min_hold` weeks in a row; with `min_hold` 1 every flip counts.
/// A week with no leader (None) neither changes nor resets the champion.
pub fn leader_changes(leaders: &[Option<u64>], min_hold: usize) -> u32 {
    let mut champion: Option<u64> = None;
    let mut challenger: Option<(u64, usize)> = None;
    let mut changes = 0;
    for leader in leaders.iter().flatten() {
        match champion {
            None => champion = Some(*leader),
            Some(current) if current == *leader => challenger = None,
            Some(_) => {
                let run = match challenger {
                    Some((id, run)) if id == *leader => run + 1,
                    _ => 1,
                };
                if run >= min_hold {
                    champion = Some(*leader);
                    challenger = None;
                    changes += 1;
                } else {
                    challenger = Some((*leader, run));
                }
            }
        }
    }
    changes
}

/// Rank (1 is best) of `handle` among `(handle, size)` rows sorted by size descending, ties by
/// handle ascending.
pub fn rank_of(rows: &[(u64, i64)], handle: u64) -> Option<u32> {
    let mut sorted: Vec<(u64, i64)> = rows.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    sorted
        .iter()
        .position(|(id, _)| *id == handle)
        .map(|index| index as u32 + 1)
}

/// True when rank `rank` of `league` firms is in the bottom half (strictly past the middle).
pub fn bottom_half(rank: u32, league: u32) -> bool {
    rank * 2 > league
}

/// (max - min) as a percent of the reference price, two decimals as integer hundredths of a
/// percent. Zero reference gives None.
pub fn range_pct_x100(min: i64, max: i64, reference: i64) -> Option<i64> {
    if reference <= 0 {
        return None;
    }
    Some((max - min) * 10_000 / reference)
}

/// Share as hundredths of a percent, with a zero denominator giving zero.
pub fn share_x100(part: u64, whole: u64) -> i64 {
    if whole == 0 {
        0
    } else {
        (part as u128 * 10_000 / whole as u128) as i64
    }
}

/// Scales a count over `weeks` to a count per 520-week decade.
pub fn per_decade(count: u32, weeks: u32) -> f64 {
    if weeks == 0 {
        0.0
    } else {
        count as f64 * 520.0 / weeks as f64
    }
}

/// Did a firm end differently? Alive status differs, or both alive and the larger size is more
/// than ten percent above the smaller.
pub fn differs(a: (bool, i64), b: (bool, i64)) -> (bool, bool) {
    if a.0 != b.0 {
        return (true, false);
    }
    if !a.0 {
        return (false, false);
    }
    let big = a.1.abs().max(b.1.abs());
    let size = (a.1 - b.1).abs() as i128 * 10 > big as i128;
    (false, size)
}

// ---------------------------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct YearEnd {
    pub year: u32,
    pub week: u32,
    pub net_worth_cents: i64,
    pub rank: Option<u32>,
    pub league_size: u32,
    pub player_equity_cents: Option<i64>,
    pub leader: Option<String>,
    pub largest_rival_cents: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HostileEvent {
    pub week: u32,
    /// raid, deal, activist or plan.
    pub kind: String,
    pub actor: String,
    pub target: String,
    /// What happened, as far as the harness could see by the end of the run.
    pub outcome: String,
    /// Blocks taken (raid), shares sought (deal) or empty.
    pub detail: String,
    pub player_equity_cents: i64,
    pub deal: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Hostile {
    pub log: Vec<HostileEvent>,
    pub raids: u32,
    pub raids_succeeded: u32,
    pub hostile_deals: u32,
    pub activist_campaigns: u32,
    pub takeover_plans: u32,
    pub total: u32,
    /// Hostile moves that landed in weeks the player ranked in the top three.
    pub while_top_three: u32,
    pub weeks_top_three: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Turnover {
    pub entries_by_year: Vec<u32>,
    pub exits_by_year: Vec<u32>,
    pub dissolved: u32,
    pub acquired: u32,
    pub wound_down: u32,
    /// Of `dissolved`: firms that were past due or restructuring the week before (insolvency).
    pub dissolved_after_distress: u32,
    /// Of `dissolved`: firms that were healthy the week before (a voluntary wind down).
    pub dissolved_voluntary: u32,
    /// Times a firm other than the player's came under a different ultimate controller (a settled
    /// deal that changed control). The firm stays in the league's books, so it is not an exit.
    pub control_changes: u32,
    pub entries: u32,
    pub exits: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ZombieFirm {
    pub name: String,
    pub region: u8,
    pub zombie_weeks: u32,
    pub first_week: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Zombies {
    pub firm_weeks: u64,
    pub zombie_weeks: u64,
    pub stressed_weeks: u64,
    pub worst: Vec<ZombieFirm>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DealCounts {
    pub attempted: u32,
    pub settled: u32,
    pub rejected: u32,
    pub withdrawn: u32,
    pub expired: u32,
    pub still_open: u32,
    pub player_attempted: u32,
    pub player_settled: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GoodPrice {
    pub good: String,
    pub reference_cents: i64,
    /// Whether the good has a gateway (traded) or not (services and retail goods: local).
    #[serde(default)]
    pub traded: bool,
    /// Median over years of (max - min) / reference, in percent.
    pub median_year_range_pct: f64,
    pub years: u32,
    pub weeks_at_cap_share_pct: f64,
    pub weeks_at_floor_share_pct: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct League {
    pub leader_changes_raw: u32,
    pub leader_changes_held_13w: u32,
    pub largest_rival_start_cents: i64,
    pub largest_rival_end_cents: i64,
    pub rivals_at_start: u32,
    pub rivals_survived: u32,
    /// Survivors' end/start valuation in percent: min, p10, median, p90, max.
    pub rival_growth_pct: Vec<i64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Influence {
    pub compared: u32,
    pub alive_differs: u32,
    pub size_differs: u32,
    pub share_differs_pct: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Career {
    pub seed: u64,
    pub weeks_played: u32,
    pub paused: Option<String>,
    pub year_ends: Vec<YearEnd>,
    pub final_rank: Option<u32>,
    pub final_league_size: u32,
    pub final_net_worth_cents: i64,
    pub had_company: bool,
    pub bust: bool,
    pub bust_week: Option<u32>,
    /// First week the primary company was in restructuring (it may recover).
    pub restructured_week: Option<u32>,
    pub lost_control_votes_bps: Option<i32>,
    pub lost_control_to: Option<String>,
    pub lost_control: bool,
    pub lost_control_week: Option<u32>,
    pub past_due_weeks: u32,
    pub best_rank: Option<u32>,
    pub hostile: Hostile,
    pub league: League,
    pub turnover: Turnover,
    pub zombies: Zombies,
    pub deals: DealCounts,
    pub goods: Vec<GoodPrice>,
    pub influence: Option<Influence>,
    /// The same comparison for the inert noise run, when one was supplied: the floor.
    pub influence_floor: Option<Influence>,
    pub policy_attempted: u32,
    pub policy_accepted: u32,
    pub policy_rejected: u32,
    pub ms_per_week: f64,
    /// Every firm known at week 0: (handle, operative at end, valuation at end). Used by a later
    /// run to compare against this one.
    pub final_firms: Vec<(u64, bool, i64)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct GateResult {
    pub id: String,
    pub text: String,
    pub metric: String,
    pub value: Option<f64>,
    /// "pass", "fail" or "n/a" (the gate belongs to another policy, or there was no data).
    pub result: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Aggregate {
    pub careers: u32,
    pub companies: u32,
    pub bottom_half_share: Option<f64>,
    pub bust_share: Option<f64>,
    pub first_place_share: Option<f64>,
    pub top_three_share: Option<f64>,
    pub lost_control_share: Option<f64>,
    pub median_final_net_worth_cents: Option<i64>,
    pub net_worth_by_year_median_cents: Vec<i64>,
    pub median_final_rank: Option<f64>,
    pub leader_changes_per_decade_median: Option<f64>,
    pub leader_changes_raw_median: Option<f64>,
    pub largest_rival_growth_pct_median: Option<f64>,
    pub rival_growth_spread_pct_median: Option<f64>,
    pub entries_per_decade_median: Option<f64>,
    pub exits_per_decade_median: Option<f64>,
    pub exits_dissolved: u32,
    pub exits_after_distress: u32,
    pub exits_voluntary: u32,
    pub control_changes_total: u32,
    pub exits_acquired: u32,
    pub exits_wound_down: u32,
    pub zombie_share: Option<f64>,
    pub stressed_share: Option<f64>,
    pub worst_zombies: Vec<ZombieFirm>,
    pub price_range_pct_median: Option<f64>,
    /// The same median over goods with a gateway only, and over goods without one (services and
    /// retail goods).
    #[serde(default)]
    pub price_range_traded_pct_median: Option<f64>,
    #[serde(default)]
    pub price_range_local_pct_median: Option<f64>,
    /// Per good: (good, traded, median range in percent over seeds).
    #[serde(default)]
    pub price_range_by_good: Vec<(String, bool, f64)>,
    pub weeks_at_cap_share_pct_median: Option<f64>,
    pub weeks_at_floor_share_pct_median: Option<f64>,
    pub hostile_total_median: Option<f64>,
    pub hostile_top3_median: Option<f64>,
    pub hostile_top3_careers: u32,
    pub deals: DealCounts,
    pub influence_share_differs_pct_median: Option<f64>,
    pub influence_floor_pct_median: Option<f64>,
    /// Influence less the floor, in percentage points: what the player moved beyond chaos.
    pub influence_net_pct_median: Option<f64>,
    pub ms_per_week_mean: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ScenarioCheck {
    pub requested: String,
    pub loaded: String,
    pub regions: usize,
    pub seed_firms: usize,
    pub world_firms: usize,
    pub currencies: usize,
    pub logistics_carriers: usize,
    pub player_region: String,
    pub player_recipe: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ArcReport {
    pub schema: String,
    pub engine_version: String,
    pub rules_version: u32,
    pub scenario: ScenarioCheck,
    pub policy: String,
    pub difficulty: String,
    pub weeks: u32,
    pub seed_from: u64,
    pub seed_count: u64,
    pub definitions: BTreeMap<String, String>,
    pub careers: Vec<Career>,
    pub aggregate: Aggregate,
    pub gates: Vec<GateResult>,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------------------------
// Observer
// ---------------------------------------------------------------------------------------------

fn valuation(firm: &cw_sim::state::Firm) -> i64 {
    if firm.valuation.computed_week.0 == 0 && firm.valuation.equity_cents == 0 {
        firm.book.total_equity()
    } else {
        firm.valuation.equity_cents
    }
}

fn workers(world: &World, firm: &cw_sim::state::Firm) -> i64 {
    firm.facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .map(|facility| facility.workers.iter().sum::<i64>())
        .sum()
}

struct Life {
    exit: bool,
}

#[derive(Default)]
struct PriceTrack {
    /// (good, year) -> (min, max, observations, sum of the week's price anchor)
    windows: BTreeMap<(u16, u32), (i64, i64, u32, i128)>,
    weeks: BTreeMap<u16, (u64, u64, u64)>,
    last_week: u32,
}

struct Observer {
    initial_done: bool,
    initial: BTreeSet<u64>,
    lives: BTreeMap<u64, Life>,
    turnover: Turnover,
    zombies: Zombies,
    zombie_by_firm: BTreeMap<u64, ZombieFirm>,
    leaders: Vec<Option<u64>>,
    start_rivals: BTreeMap<u64, i64>,
    largest_rival_start: i64,
    year_ends: Vec<YearEnd>,
    had_company: bool,
    bust_week: Option<u32>,
    lost_control_week: Option<u32>,
    past_due_weeks: u32,
    best_rank: Option<u32>,
    hostile: Hostile,
    seen_raids: BTreeSet<(u32, u64, u64, u8, u8)>,
    seen_deals: BTreeSet<(u64, u32)>,
    seen_campaigns: BTreeSet<u32>,
    seen_plans: BTreeSet<(u64, u64, u32)>,
    deals: BTreeMap<u64, (DealState, bool)>,
    prices: PriceTrack,
    hook_seconds: f64,
    last_primary: Option<Handle>,
    lost_control_votes_bps: Option<i32>,
    lost_control_to: Option<String>,
    restructured_week: Option<u32>,
    prev_status: BTreeMap<u64, FirmStatus>,
    last_controller: BTreeMap<u64, Option<HolderId>>,
}

impl Observer {
    fn new() -> Observer {
        Observer {
            initial_done: false,
            initial: BTreeSet::new(),
            lives: BTreeMap::new(),
            turnover: Turnover::default(),
            zombies: Zombies::default(),
            zombie_by_firm: BTreeMap::new(),
            leaders: Vec::new(),
            start_rivals: BTreeMap::new(),
            largest_rival_start: 0,
            year_ends: Vec::new(),
            had_company: false,
            bust_week: None,
            lost_control_week: None,
            past_due_weeks: 0,
            best_rank: None,
            hostile: Hostile::default(),
            seen_raids: BTreeSet::new(),
            seen_deals: BTreeSet::new(),
            seen_campaigns: BTreeSet::new(),
            seen_plans: BTreeSet::new(),
            deals: BTreeMap::new(),
            prices: PriceTrack::default(),
            hook_seconds: 0.0,
            last_primary: None,
            lost_control_votes_bps: None,
            lost_control_to: None,
            restructured_week: None,
            prev_status: BTreeMap::new(),
            last_controller: BTreeMap::new(),
        }
    }

    /// The league rows `(handle, valuation)` and the player's group.
    fn league(world: &World) -> (Vec<(u64, i64)>, BTreeSet<u64>) {
        let primary = world.investor.primary_group;
        let mut rows = Vec::new();
        let mut group = BTreeSet::new();
        for (handle, firm) in world.firms.iter() {
            let controller = ultimate_controller(world, handle);
            if controller == Some(HolderId::Investor) {
                group.insert(handle.0);
            }
            if !firm.status.operative() {
                continue;
            }
            let top = !matches!(controller, Some(HolderId::Firm(_)));
            let ours = controller == Some(HolderId::Investor);
            if top && (!ours || Some(handle) == primary) {
                rows.push((handle.0, valuation(firm)));
            }
        }
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        (rows, group)
    }

    fn observe(&mut self, session: &Session, after_week: bool) {
        let started = Instant::now();
        let world = &session.world;
        let week = world.tick.0;
        let year_index = week.saturating_sub(1) / YEAR;
        let primary = world.investor.primary_group;
        let (rows, group) = Self::league(world);
        let player_rank = primary.and_then(|p| {
            if world.firm(p).is_some_and(|f| f.status.operative()) {
                rank_of(&rows, p.0)
            } else {
                None
            }
        });

        // Firms: entries, exits, zombies.
        for (handle, firm) in world.firms.iter() {
            let id = handle.0;
            if let std::collections::btree_map::Entry::Vacant(slot) = self.lives.entry(id) {
                slot.insert(Life { exit: false });
                if !self.initial_done {
                    self.initial.insert(id);
                } else if after_week && !group.contains(&id) {
                    self.turnover.entries += 1;
                    bump(&mut self.turnover.entries_by_year, year_index);
                }
            }
            let kind = match firm.status {
                FirmStatus::Merged { .. } => Some("acquired"),
                FirmStatus::Dissolved => Some("dissolved"),
                FirmStatus::Liquidating => Some("wound_down"),
                _ => None,
            };
            if let Some(kind) = kind
                && let Some(life) = self.lives.get_mut(&id)
                && !life.exit
            {
                life.exit = true;
                if after_week {
                    self.turnover.exits += 1;
                    bump(&mut self.turnover.exits_by_year, year_index);
                    match kind {
                        "acquired" => self.turnover.acquired += 1,
                        "dissolved" => {
                            self.turnover.dissolved += 1;
                            // The engine liquidates and dissolves inside one week, so the status
                            // never reads Liquidating; the week before tells the two causes apart.
                            if matches!(
                                self.prev_status.get(&id),
                                Some(FirmStatus::PastDue { .. } | FirmStatus::Restructuring { .. })
                            ) {
                                self.turnover.dissolved_after_distress += 1;
                            } else {
                                self.turnover.dissolved_voluntary += 1;
                            }
                        }
                        _ => self.turnover.wound_down += 1,
                    }
                }
            }
            if after_week && firm.status.operative() && !group.contains(&id) {
                let controller = ultimate_controller(world, handle);
                match self.last_controller.insert(id, controller) {
                    Some(before) if before != controller => self.turnover.control_changes += 1,
                    _ => {}
                }
            }
            if after_week {
                self.prev_status.insert(id, firm.status);
            }
            if after_week && firm.status.operative() {
                self.zombies.firm_weeks += 1;
                let cash = firm.book.cash();
                if cash <= 0 && workers(world, firm) == 0 {
                    self.zombies.zombie_weeks += 1;
                    let entry = self.zombie_by_firm.entry(id).or_insert(ZombieFirm {
                        name: firm.name.clone(),
                        region: firm.region,
                        zombie_weeks: 0,
                        first_week: week,
                    });
                    entry.zombie_weeks += 1;
                }
                let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, handle);
                let revenue = firm
                    .book
                    .recent_weeks()
                    .last()
                    .map(|w| w.revenue)
                    .unwrap_or(0);
                if cash < outflow * 4 || revenue == 0 {
                    self.zombies.stressed_weeks += 1;
                }
            }
        }

        // The player. The engine clears the primary group when its company is wound up, so the
        // last primary is followed until a replacement is founded.
        if primary.is_some() {
            self.last_primary = primary;
        }
        if let Some(p) = self.last_primary
            && let Some(firm) = world.firm(p)
        {
            self.had_company = true;
            if after_week {
                if matches!(firm.status, FirmStatus::Liquidating | FirmStatus::Dissolved)
                    && self.bust_week.is_none()
                {
                    self.bust_week = Some(week);
                }
                if matches!(firm.status, FirmStatus::Restructuring { .. })
                    && self.restructured_week.is_none()
                {
                    self.restructured_week = Some(week);
                }
                if matches!(firm.status, FirmStatus::PastDue { .. }) {
                    self.past_due_weeks += 1;
                }
                let controlled = ultimate_controller(world, p) == Some(HolderId::Investor);
                let wound_up =
                    matches!(firm.status, FirmStatus::Liquidating | FirmStatus::Dissolved);
                if !controlled && !wound_up && self.lost_control_week.is_none() {
                    self.lost_control_week = Some(week);
                    self.lost_control_votes_bps = Some(firm.shares.vote_bps(HolderId::Investor));
                    self.lost_control_to = Some(
                        match ultimate_controller(world, p) {
                            None => "nobody (below a majority)",
                            Some(HolderId::Firm(_)) => "a rival firm",
                            Some(HolderId::Outside(_)) => "an outside holder",
                            Some(HolderId::State) | Some(HolderId::Treasury) => "the state",
                            Some(HolderId::Investor) => "investor",
                        }
                        .to_owned(),
                    );
                }
            }
        }
        if let Some(rank) = player_rank {
            self.best_rank = Some(self.best_rank.map_or(rank, |b| b.min(rank)));
        }

        if !after_week {
            // Opening snapshot: the league's starting sizes.
            for (id, size) in &rows {
                if Some(Handle(*id)) != primary {
                    self.start_rivals.insert(*id, *size);
                }
            }
            self.largest_rival_start = self.start_rivals.values().copied().max().unwrap_or(0);
            self.initial_done = true;
            self.hook_seconds += started.elapsed().as_secs_f64();
            return;
        }

        // League leader.
        self.leaders.push(rows.first().map(|r| r.0));

        // Hostile moves aimed at the player's group.
        let mut fresh = 0;
        let name_of = |h: Handle| {
            world
                .firm(h)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| format!("firm {}", h.0))
        };
        let player_equity = primary
            .and_then(|p| world.firm(p))
            .map(valuation)
            .unwrap_or(0);
        if !group.is_empty() {
            let mut plan_deals: BTreeSet<u64> = BTreeSet::new();
            for plan in &world.plans.plans {
                if let Some(deal) = plan.deal {
                    plan_deals.insert(deal.0);
                }
                if group.contains(&plan.target.0)
                    && !group.contains(&plan.raider.0)
                    && self
                        .seen_plans
                        .insert((plan.raider.0, plan.target.0, plan.started))
                {
                    self.hostile.takeover_plans += 1;
                    fresh += 1;
                    self.hostile.log.push(HostileEvent {
                        week,
                        kind: "plan".to_owned(),
                        actor: name_of(plan.raider),
                        target: name_of(plan.target),
                        outcome: format!("{:?}", plan.stage),
                        detail: String::new(),
                        player_equity_cents: player_equity,
                        deal: plan.deal.map(|d| d.0),
                    });
                }
            }
            for raid in &world.raids {
                if group.contains(&raid.defender.0)
                    && !group.contains(&raid.attacker.0)
                    && self.seen_raids.insert((
                        raid.tick,
                        raid.attacker.0,
                        raid.defender.0,
                        raid.region,
                        raid.industry,
                    ))
                {
                    self.hostile.raids += 1;
                    if raid.success {
                        self.hostile.raids_succeeded += 1;
                    }
                    fresh += 1;
                    self.hostile.log.push(HostileEvent {
                        week,
                        kind: "raid".to_owned(),
                        actor: name_of(raid.attacker),
                        target: name_of(raid.defender),
                        outcome: if raid.success {
                            format!(
                                "won: took {} block(s), paid the player {} cents",
                                raid.blocks, raid.price_cents
                            )
                        } else {
                            "failed".to_owned()
                        },
                        detail: format!(
                            "chance {} bps, budget {} cents, region {}",
                            raid.chance_bps, raid.budget_cents, raid.region
                        ),
                        player_equity_cents: player_equity,
                        deal: None,
                    });
                }
            }
            for (handle, deal) in world.deals.iter() {
                if deal.hostile
                    && group.contains(&deal.target.0)
                    && !group.contains(&deal.buyer.0)
                    && !plan_deals.contains(&handle.0)
                    && self.seen_deals.insert((handle.0, deal.started.0))
                {
                    self.hostile.hostile_deals += 1;
                    fresh += 1;
                    self.hostile.log.push(HostileEvent {
                        week,
                        kind: "deal".to_owned(),
                        actor: name_of(deal.buyer),
                        target: name_of(deal.target),
                        outcome: format!("{:?}", deal.state),
                        detail: format!(
                            "{} shares at {} cents",
                            deal.max_shares, deal.offer_price_cents
                        ),
                        player_equity_cents: player_equity,
                        deal: Some(handle.0),
                    });
                }
            }
            for campaign in &world.structure.campaigns {
                if group.contains(&campaign.target.0) && self.seen_campaigns.insert(campaign.id) {
                    self.hostile.activist_campaigns += 1;
                    fresh += 1;
                    self.hostile.log.push(HostileEvent {
                        week,
                        kind: "activist".to_owned(),
                        actor: format!("outside activist {}", campaign.activist),
                        target: name_of(campaign.target),
                        outcome: format!("{:?}", campaign.outcome),
                        detail: format!(
                            "{:?}, demand {:?}, stake {} bps, {} cents",
                            campaign.reason,
                            campaign.demand,
                            campaign.stake_bps,
                            campaign.amount_cents
                        ),
                        player_equity_cents: player_equity,
                        deal: None,
                    });
                }
            }
        }
        self.hostile.total += fresh;
        if player_rank.is_some_and(|rank| rank <= 3) {
            self.hostile.while_top_three += fresh;
            self.hostile.weeks_top_three += 1;
        }

        // Deals.
        for (handle, deal) in world.deals.iter() {
            self.deals
                .insert(handle.0, (deal.state, group.contains(&deal.buyer.0)));
        }

        // Prices.
        if let Some(point) = world.price_history.last()
            && point.week != self.prices.last_week
        {
            self.prices.last_week = point.week;
            let year = point.week.saturating_sub(1) / YEAR;
            let mut sell: BTreeMap<u16, i64> = BTreeMap::new();
            let mut buy: BTreeMap<u16, i64> = BTreeMap::new();
            for gateway in &world.gateways {
                if gateway.sell_price_cents > 0 {
                    let slot = sell.entry(gateway.product).or_insert(i64::MAX);
                    *slot = (*slot).min(gateway.sell_price_cents);
                }
                if gateway.buy_price_cents > 0 {
                    let slot = buy.entry(gateway.product).or_insert(0);
                    *slot = (*slot).max(gateway.buy_price_cents);
                }
            }
            for (good, price) in &point.goods {
                let anchor = cw_sim::economy::pricing::price_anchor(world, &session.dataset, *good);
                let window =
                    self.prices
                        .windows
                        .entry((*good, year))
                        .or_insert((i64::MAX, i64::MIN, 0, 0));
                window.0 = window.0.min(*price);
                window.1 = window.1.max(*price);
                window.2 += 1;
                window.3 += anchor as i128;
                let counts = self.prices.weeks.entry(*good).or_default();
                counts.0 += 1;
                if sell.get(good).is_some_and(|cap| *price >= *cap) {
                    counts.1 += 1;
                }
                if buy.get(good).is_some_and(|floor| *price <= *floor) {
                    counts.2 += 1;
                }
            }
        }

        // Year end.
        if week.is_multiple_of(YEAR) {
            let rivals_largest = rows
                .iter()
                .find(|(id, _)| Some(Handle(*id)) != primary)
                .map(|r| r.1)
                .unwrap_or(0);
            self.year_ends.push(YearEnd {
                year: week / YEAR,
                week,
                net_worth_cents: session.overview().investor_net_worth_cents,
                rank: player_rank,
                league_size: rows.len() as u32,
                player_equity_cents: primary.and_then(|p| world.firm(p)).map(valuation),
                leader: rows
                    .first()
                    .and_then(|r| world.firm(Handle(r.0)))
                    .map(|f| f.name.clone()),
                largest_rival_cents: rivals_largest,
            });
        }
        self.hook_seconds += started.elapsed().as_secs_f64();
    }
}

fn bump(counts: &mut Vec<u32>, index: u32) {
    let index = index as usize;
    if counts.len() <= index {
        counts.resize(index + 1, 0);
    }
    counts[index] += 1;
}

fn finish_career(
    seed: u64,
    session: &Session,
    observer: Observer,
    runner: &PolicyRunner,
    advanced: u32,
    paused: Option<String>,
    engine_seconds: f64,
) -> Career {
    let world = &session.world;
    let (rows, group) = Observer::league(world);
    let primary = world.investor.primary_group;
    let operative = primary
        .and_then(|p| world.firm(p))
        .is_some_and(|f| f.status.operative());
    let final_rank = primary
        .filter(|_| operative)
        .and_then(|p| rank_of(&rows, p.0));
    let lost = observer.lost_control_week.is_some();

    // Rival growth.
    let mut growth: Vec<i64> = Vec::new();
    let mut survived = 0;
    let end_value: BTreeMap<u64, i64> = rows.iter().copied().collect();
    for (id, start) in &observer.start_rivals {
        if let Some(end) = end_value.get(id) {
            survived += 1;
            if *start > 0 {
                growth.push(*end as i128 as i64 * 100 / *start);
            }
        }
    }
    let spread = if growth.is_empty() {
        Vec::new()
    } else {
        vec![
            growth.iter().copied().min().unwrap_or(0),
            quantile_i64(&growth, 1, 10).unwrap_or(0),
            median_i64(&growth).unwrap_or(0),
            quantile_i64(&growth, 9, 10).unwrap_or(0),
            growth.iter().copied().max().unwrap_or(0),
        ]
    };
    let largest_end = rows
        .iter()
        .filter(|(id, _)| Some(Handle(*id)) != primary)
        .map(|r| r.1)
        .max()
        .unwrap_or(0);

    // Zombies.
    let mut zombies = observer.zombies.clone();
    let mut worst: Vec<ZombieFirm> = observer.zombie_by_firm.values().cloned().collect();
    worst.sort_by(|a, b| {
        b.zombie_weeks
            .cmp(&a.zombie_weeks)
            .then(a.name.cmp(&b.name))
    });
    worst.truncate(5);
    zombies.worst = worst;

    // Hostile deals and plans: the state the deal ended in.
    let mut hostile = observer.hostile.clone();
    for event in &mut hostile.log {
        if let Some(deal) = event.deal
            && let Some((state, _)) = observer.deals.get(&deal)
        {
            event.outcome = format!("{} (deal ended {:?})", event.outcome, state);
        }
    }

    // Deals.
    let mut deals = DealCounts::default();
    for (state, player) in observer.deals.values() {
        deals.attempted += 1;
        if *player {
            deals.player_attempted += 1;
        }
        match state {
            DealState::Settled => {
                deals.settled += 1;
                if *player {
                    deals.player_settled += 1;
                }
            }
            DealState::Rejected => deals.rejected += 1,
            DealState::Withdrawn => deals.withdrawn += 1,
            DealState::Expired => deals.expired += 1,
            _ => deals.still_open += 1,
        }
    }

    // Prices.
    let mut goods = Vec::new();
    let mut by_good: BTreeMap<u16, Vec<f64>> = BTreeMap::new();
    for ((good, _year), (min, max, count, anchor_sum)) in &observer.prices.windows {
        if *count < 13 {
            continue;
        }
        // Range against the year's mean anchor: the outside middle quote for a traded good, the
        // price-level reference for a service, the catalogue price otherwise. A world price
        // path that drifts is not counted as range.
        let reference = (*anchor_sum / *count as i128).max(1) as i64;
        if let Some(range) = range_pct_x100(*min, *max, reference) {
            by_good.entry(*good).or_default().push(range as f64 / 100.0);
        }
    }
    for (good, ranges) in &by_good {
        let (weeks, cap, floor) = observer.prices.weeks.get(good).copied().unwrap_or_default();
        goods.push(GoodPrice {
            good: session.dataset.products[*good as usize].id.clone(),
            reference_cents: session.dataset.reference_price(*good),
            traded: !session.dataset.products[*good as usize].kind.is_service()
                && session.world.gateways.iter().any(|g| {
                    g.product == *good && (g.sell_price_cents > 0 || g.buy_price_cents > 0)
                }),
            median_year_range_pct: median_f64(ranges).unwrap_or(0.0),
            years: ranges.len() as u32,
            weeks_at_cap_share_pct: share_x100(cap, weeks) as f64 / 100.0,
            weeks_at_floor_share_pct: share_x100(floor, weeks) as f64 / 100.0,
        });
    }

    let final_firms = observer
        .initial
        .iter()
        .filter_map(|id| {
            world
                .firm(Handle(*id))
                .map(|f| (*id, f.status.operative(), valuation(f)))
        })
        .collect();

    let _ = group;
    Career {
        seed,
        weeks_played: advanced,
        paused,
        final_rank,
        final_league_size: rows.len() as u32,
        final_net_worth_cents: session.overview().investor_net_worth_cents,
        had_company: observer.had_company,
        bust: observer.bust_week.is_some(),
        bust_week: observer.bust_week,
        restructured_week: observer.restructured_week,
        lost_control_votes_bps: observer.lost_control_votes_bps,
        lost_control_to: observer.lost_control_to.clone(),
        lost_control: lost,
        lost_control_week: observer.lost_control_week,
        past_due_weeks: observer.past_due_weeks,
        best_rank: observer.best_rank,
        hostile,
        league: League {
            leader_changes_raw: leader_changes(&observer.leaders, 1),
            leader_changes_held_13w: leader_changes(&observer.leaders, LEADER_HOLD_WEEKS),
            largest_rival_start_cents: observer.largest_rival_start,
            largest_rival_end_cents: largest_end,
            rivals_at_start: observer.start_rivals.len() as u32,
            rivals_survived: survived,
            rival_growth_pct: spread,
        },
        turnover: observer.turnover.clone(),
        zombies,
        deals,
        goods,
        influence: None,
        influence_floor: None,
        policy_attempted: runner.stats.attempted,
        policy_accepted: runner.stats.accepted,
        policy_rejected: runner.stats.rejected,
        ms_per_week: if advanced == 0 {
            0.0
        } else {
            engine_seconds * 1000.0 / advanced as f64
        },
        year_ends: observer.year_ends,
        final_firms,
    }
}

// ---------------------------------------------------------------------------------------------
// Running
// ---------------------------------------------------------------------------------------------

pub fn arc_config_pub(seed: u64, scenario_id: &str) -> ScenarioConfig {
    arc_config(seed, scenario_id, cw_data::Difficulty::Standard)
}

pub(crate) fn arc_config(
    seed: u64,
    scenario_id: &str,
    difficulty: cw_data::Difficulty,
) -> ScenarioConfig {
    // The new-game screen's own path: founder, sandbox, the scenario named in the price setup
    // (which loads that scenario's dataset: regions, firms, logistics, currencies).
    ScenarioConfig {
        seed,
        scenario: cw_sim::state::ScenarioKind::Founder,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario_id.to_owned(),
            path: None,
            timeline: None,
        },
    }
}

fn scenario_check(session: &Session, requested: &str) -> ScenarioCheck {
    ScenarioCheck {
        requested: requested.to_owned(),
        loaded: session.dataset.scenario_id.clone(),
        regions: session.dataset.regions.len(),
        seed_firms: session.dataset.seed_firms.len(),
        world_firms: session.world.firms.len(),
        currencies: session.world.fx.currencies.len(),
        logistics_carriers: session.world.logistics.carriers.len(),
        player_region: session.dataset.player_region.to_owned(),
        player_recipe: session.dataset.player_recipe.to_owned(),
    }
}

/// One career with the observer attached.
fn play(
    seed: u64,
    weeks: u32,
    scenario_id: &str,
    difficulty: cw_data::Difficulty,
    policy: ReferencePolicy,
) -> Result<(Career, ScenarioCheck), String> {
    let mut session = Session::create(arc_config(seed, scenario_id, difficulty))
        .map_err(|e| format!("seed {seed}: session failed: {e:?}"))?;
    apply_markets_override(&mut session)?;
    let check = scenario_check(&session, scenario_id);
    let mut runner = PolicyRunner::new(policy, VERBOSE.load(std::sync::atomic::Ordering::Relaxed));
    let mut observer = Observer::new();
    let wall = Instant::now();
    let (advanced, paused) =
        crate::drive_with(&mut session, weeks, &mut runner, &mut |s, after| {
            observer.observe(s, after)
        });
    let engine_seconds = (wall.elapsed().as_secs_f64() - observer.hook_seconds).max(0.0);
    let career = finish_career(
        seed,
        &session,
        observer,
        &runner,
        advanced,
        paused,
        engine_seconds,
    );
    Ok((career, check))
}

/// The cheap end state of a `none` run: only the final firms.
fn play_none(
    seed: u64,
    weeks: u32,
    scenario_id: &str,
    difficulty: cw_data::Difficulty,
) -> Result<Vec<(u64, bool, i64)>, String> {
    play(seed, weeks, scenario_id, difficulty, ReferencePolicy::None).map(|(c, _)| c.final_firms)
}

fn compare(none: &[(u64, bool, i64)], policy: &[(u64, bool, i64)]) -> Influence {
    let by_policy: BTreeMap<u64, (bool, i64)> =
        policy.iter().map(|(h, a, v)| (*h, (*a, *v))).collect();
    let mut out = Influence::default();
    for (handle, alive, size) in none {
        let Some(other) = by_policy.get(handle) else {
            continue;
        };
        out.compared += 1;
        let (alive_differs, size_differs) = differs((*alive, *size), *other);
        out.alive_differs += alive_differs as u32;
        out.size_differs += size_differs as u32;
    }
    out.share_differs_pct = if out.compared == 0 {
        0.0
    } else {
        share_x100(
            (out.alive_differs + out.size_differs) as u64,
            out.compared as u64,
        ) as f64
            / 100.0
    };
    out
}

// ---------------------------------------------------------------------------------------------
// Aggregation and gates
// ---------------------------------------------------------------------------------------------

fn share(part: usize, whole: usize) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

fn med(values: Vec<f64>) -> Option<f64> {
    median_f64(&values)
}

pub fn aggregate(careers: &[Career]) -> Aggregate {
    let with_company: Vec<&Career> = careers.iter().filter(|c| c.had_company).collect();
    let n = with_company.len();
    let bottom = with_company
        .iter()
        .filter(|c| match c.final_rank {
            Some(rank) => bottom_half(rank, c.final_league_size.max(1)),
            None => true,
        })
        .count();
    let bust = with_company.iter().filter(|c| c.bust).count();
    let first = with_company
        .iter()
        .filter(|c| c.final_rank == Some(1))
        .count();
    let top3 = with_company
        .iter()
        .filter(|c| c.final_rank.is_some_and(|r| r <= 3))
        .count();
    let lost = with_company.iter().filter(|c| c.lost_control).count();

    let years = careers.iter().map(|c| c.year_ends.len()).max().unwrap_or(0);
    let mut by_year = Vec::new();
    for year in 0..years {
        let values: Vec<i64> = with_company
            .iter()
            .filter_map(|c| c.year_ends.get(year).map(|y| y.net_worth_cents))
            .collect();
        by_year.push(median_i64(&values).unwrap_or(0));
    }

    let mut deals = DealCounts::default();
    let (mut firm_weeks, mut zombie_weeks, mut stressed) = (0u64, 0u64, 0u64);
    let mut worst: Vec<ZombieFirm> = Vec::new();
    let (mut dissolved, mut acquired, mut wound) = (0, 0, 0);
    let (mut distress, mut voluntary, mut control_changes) = (0, 0, 0);
    for c in careers {
        deals.attempted += c.deals.attempted;
        deals.settled += c.deals.settled;
        deals.rejected += c.deals.rejected;
        deals.withdrawn += c.deals.withdrawn;
        deals.expired += c.deals.expired;
        deals.still_open += c.deals.still_open;
        deals.player_attempted += c.deals.player_attempted;
        deals.player_settled += c.deals.player_settled;
        firm_weeks += c.zombies.firm_weeks;
        zombie_weeks += c.zombies.zombie_weeks;
        stressed += c.zombies.stressed_weeks;
        for z in &c.zombies.worst {
            let mut named = z.clone();
            named.name = format!("{} (seed {})", z.name, c.seed);
            worst.push(named);
        }
        dissolved += c.turnover.dissolved;
        acquired += c.turnover.acquired;
        wound += c.turnover.wound_down;
        distress += c.turnover.dissolved_after_distress;
        voluntary += c.turnover.dissolved_voluntary;
        control_changes += c.turnover.control_changes;
    }
    worst.sort_by(|a, b| {
        b.zombie_weeks
            .cmp(&a.zombie_weeks)
            .then(a.name.cmp(&b.name))
    });
    worst.truncate(5);

    // Prices: per good, median over seeds of the career median range; then the median over goods.
    type GoodSeries = (Vec<f64>, Vec<f64>, Vec<f64>);
    let mut per_good: BTreeMap<String, GoodSeries> = BTreeMap::new();
    let mut traded_goods: BTreeMap<String, bool> = BTreeMap::new();
    for c in careers {
        for g in &c.goods {
            *traded_goods.entry(g.good.clone()).or_default() |= g.traded;
            let slot = per_good.entry(g.good.clone()).or_default();
            slot.0.push(g.median_year_range_pct);
            slot.1.push(g.weeks_at_cap_share_pct);
            slot.2.push(g.weeks_at_floor_share_pct);
        }
    }
    let good_range: Vec<f64> = per_good.values().filter_map(|v| median_f64(&v.0)).collect();
    let by_good: Vec<(String, bool, f64)> = per_good
        .iter()
        .filter_map(|(good, v)| median_f64(&v.0).map(|r| (good.clone(), traded_goods[good], r)))
        .collect();
    let class_range = |traded: bool| -> Vec<f64> {
        by_good
            .iter()
            .filter(|(_, t, _)| *t == traded)
            .map(|(_, _, r)| *r)
            .collect()
    };
    let (traded_range, local_range) = (class_range(true), class_range(false));
    let good_cap: Vec<f64> = per_good.values().filter_map(|v| median_f64(&v.1)).collect();
    let good_floor: Vec<f64> = per_good.values().filter_map(|v| median_f64(&v.2)).collect();

    let hostile_top3: Vec<f64> = with_company
        .iter()
        .filter(|c| c.hostile.weeks_top_three > 0)
        .map(|c| per_decade(c.hostile.while_top_three, c.weeks_played))
        .collect();

    let growth_median: Vec<f64> = careers
        .iter()
        .filter(|c| c.league.largest_rival_start_cents > 0)
        .map(|c| {
            c.league.largest_rival_end_cents as f64 * 100.0
                / c.league.largest_rival_start_cents as f64
        })
        .collect();
    let spread: Vec<f64> = careers
        .iter()
        .filter(|c| c.league.rival_growth_pct.len() == 5)
        .map(|c| (c.league.rival_growth_pct[3] - c.league.rival_growth_pct[1]) as f64)
        .collect();

    Aggregate {
        careers: careers.len() as u32,
        companies: n as u32,
        bottom_half_share: share(bottom, n),
        bust_share: share(bust, n),
        first_place_share: share(first, n),
        top_three_share: share(top3, n),
        lost_control_share: share(lost, n),
        median_final_net_worth_cents: median_i64(
            &with_company
                .iter()
                .map(|c| c.final_net_worth_cents)
                .collect::<Vec<_>>(),
        ),
        net_worth_by_year_median_cents: by_year,
        median_final_rank: med(with_company
            .iter()
            .filter_map(|c| c.final_rank.map(|r| r as f64))
            .collect()),
        leader_changes_per_decade_median: med(careers
            .iter()
            .map(|c| per_decade(c.league.leader_changes_held_13w, c.weeks_played))
            .collect()),
        leader_changes_raw_median: med(careers
            .iter()
            .map(|c| per_decade(c.league.leader_changes_raw, c.weeks_played))
            .collect()),
        largest_rival_growth_pct_median: med(growth_median),
        rival_growth_spread_pct_median: med(spread),
        entries_per_decade_median: med(careers
            .iter()
            .map(|c| per_decade(c.turnover.entries, c.weeks_played))
            .collect()),
        exits_per_decade_median: med(careers
            .iter()
            .map(|c| per_decade(c.turnover.exits, c.weeks_played))
            .collect()),
        exits_dissolved: dissolved,
        exits_after_distress: distress,
        exits_voluntary: voluntary,
        control_changes_total: control_changes,
        exits_acquired: acquired,
        exits_wound_down: wound,
        zombie_share: (firm_weeks > 0).then(|| zombie_weeks as f64 / firm_weeks as f64),
        stressed_share: (firm_weeks > 0).then(|| stressed as f64 / firm_weeks as f64),
        worst_zombies: worst,
        price_range_pct_median: median_f64(&good_range),
        price_range_traded_pct_median: median_f64(&traded_range),
        price_range_local_pct_median: median_f64(&local_range),
        price_range_by_good: by_good,
        weeks_at_cap_share_pct_median: median_f64(&good_cap),
        weeks_at_floor_share_pct_median: median_f64(&good_floor),
        hostile_total_median: med(careers
            .iter()
            .map(|c| per_decade(c.hostile.total, c.weeks_played))
            .collect()),
        hostile_top3_median: median_f64(&hostile_top3),
        hostile_top3_careers: hostile_top3.len() as u32,
        deals,
        influence_share_differs_pct_median: med(careers
            .iter()
            .filter_map(|c| c.influence.as_ref().map(|i| i.share_differs_pct))
            .collect()),
        influence_floor_pct_median: med(careers
            .iter()
            .filter_map(|c| c.influence_floor.as_ref().map(|i| i.share_differs_pct))
            .collect()),
        influence_net_pct_median: {
            let net: Vec<f64> = careers
                .iter()
                .filter_map(|c| match (&c.influence, &c.influence_floor) {
                    (Some(a), Some(f)) => Some(a.share_differs_pct - f.share_differs_pct),
                    _ => None,
                })
                .collect();
            med(net)
        },
        ms_per_week_mean: if careers.is_empty() {
            0.0
        } else {
            careers.iter().map(|c| c.ms_per_week).sum::<f64>() / careers.len() as f64
        },
    }
}

#[derive(Deserialize)]
struct GateFile {
    gates: Vec<GateSpec>,
}

#[derive(Deserialize)]
struct GateSpec {
    id: String,
    policy: String,
    metric: String,
    text: String,
    min: Option<f64>,
    max: Option<f64>,
    max_exclusive: Option<f64>,
}

fn metric_value(aggregate: &Aggregate, metric: &str) -> Option<f64> {
    match metric {
        "bottom_half_share" => aggregate.bottom_half_share,
        "bust_share" => aggregate.bust_share,
        "first_place_share" => aggregate.first_place_share,
        "leader_changes_per_decade_median" => aggregate.leader_changes_per_decade_median,
        "zombie_share" => aggregate.zombie_share,
        "exits_per_decade_median" => aggregate.exits_per_decade_median,
        "entries_per_decade_median" => aggregate.entries_per_decade_median,
        "price_range_pct_median" => aggregate.price_range_pct_median,
        "price_range_traded_pct_median" => aggregate.price_range_traded_pct_median,
        "price_range_local_pct_median" => aggregate.price_range_local_pct_median,
        "hostile_top3_median" => aggregate.hostile_top3_median,
        _ => None,
    }
}

pub fn evaluate_gates(
    gates_json: &str,
    policy: &str,
    aggregate: &Aggregate,
) -> Result<Vec<GateResult>, String> {
    let file: GateFile = serde_json::from_str(gates_json).map_err(|e| format!("gates: {e}"))?;
    Ok(file
        .gates
        .iter()
        .map(|gate| {
            let applies = gate.policy == "any" || gate.policy == policy;
            let value = metric_value(aggregate, &gate.metric);
            let result = match (applies, value) {
                (false, _) | (true, None) => "n/a",
                (true, Some(v)) => {
                    let ok = gate.min.is_none_or(|m| v >= m)
                        && gate.max.is_none_or(|m| v <= m)
                        && gate.max_exclusive.is_none_or(|m| v < m);
                    if ok { "pass" } else { "fail" }
                }
            };
            GateResult {
                id: gate.id.clone(),
                text: gate.text.clone(),
                metric: gate.metric.clone(),
                value: if applies { value } else { None },
                result: result.to_owned(),
            }
        })
        .collect())
}

/// Zero every wall-clock field so two runs of one seed compare equal.
#[cfg(test)]
pub fn strip_timing(report: &mut ArcReport) {
    for career in &mut report.careers {
        career.ms_per_week = 0.0;
    }
    report.aggregate.ms_per_week_mean = 0.0;
}

fn definitions() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for (k, v) in [
        (
            "dissolved_split",
            "dissolved = the firm's status read Dissolved. The engine liquidates and dissolves inside one week, so Liquidating is never seen and wound_down stays 0. dissolved_after_distress = it was past due or restructuring the week before (insolvency); dissolved_voluntary = it was healthy the week before (a wind down)",
        ),
        (
            "control_changes",
            "weeks in which a firm outside the player's group had a different ultimate controller than the week before (a settled share deal that moved control). The firm stays in the league's books; the engine only sets Merged through the player's ProposeMerger command",
        ),
        (
            "influence_floor",
            "influence of the inert `noise` policy (one random draw shifted at week 0) against none; influence_net = policy influence less that floor",
        ),
        (
            "size",
            "firm.valuation.equity_cents (engine valuation); book equity before the first valuation",
        ),
        (
            "league",
            "operative firms no other firm controls; the player's other investor-controlled firms are folded into the player",
        ),
        (
            "rank",
            "the player's primary company's place in the league by size; none when it is not operative",
        ),
        (
            "bust",
            "primary company entered liquidation or dissolution (restructuring is recorded separately as restructured_week; the firm may recover)",
        ),
        (
            "lost_control",
            "primary company is alive but its ultimate controller is no longer the investor; lost_control_to says who, lost_control_votes_bps the investor's votes",
        ),
        (
            "hostile",
            "raids, hostile deals (not already counted as a plan's tender), activist campaigns and rival takeover plans aimed at the player's group",
        ),
        (
            "leader_changes",
            "first place changes hands; held_13w needs the new leader to hold 13 weeks in a row",
        ),
        (
            "exits",
            "dissolved: dissolved; acquired: merged into another firm; wound_down: entered liquidation. Counted once at the first such week",
        ),
        (
            "entries",
            "firms that appear after week 0, excluding the player's group",
        ),
        (
            "zombie",
            "operative firm-week with book cash at or below zero and no workers",
        ),
        (
            "stressed",
            "diagnose rule: cash under four weeks of mandatory outflow, or no revenue last week",
        ),
        (
            "price_range",
            "per good and year (max - min) of weekly closing price over the year's mean price anchor (outside middle quote for a good with a gateway, price-level reference for a service, catalogue price otherwise), median over years then seeds then goods; split into traded (has a gateway) and local (services and goods with none)",
        ),
        (
            "cap_floor",
            "closing price at or above the cheapest gateway sell price, or at or below the dearest gateway buy price",
        ),
        (
            "influence",
            "same seed with policy none; firms known at week 0 that ended alive-differently or more than 10 percent apart in size, over firms compared. Includes chaos noise: any extra firm shifts later random draws",
        ),
        (
            "speed",
            "wall milliseconds per simulated week including the policy's turn, excluding the observer",
        ),
    ] {
        m.insert(k.to_owned(), v.to_owned());
    }
    m
}

// ---------------------------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------------------------

fn flag(arguments: &[String], name: &str) -> Option<String> {
    crate::flag(arguments, name)
}

/// Print every policy command as it is accepted or rejected (`--verbose`).
static VERBOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `--markets-json PATH`: market rule fields that replace the content's for this run (developer
/// experiments only; the career's dataset hash is not changed, so such a run is not a replay).
static MARKETS_OVERRIDE: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

pub(crate) fn load_markets_override(arguments: &[String]) {
    if let Some(path) = flag(arguments, "--markets-json") {
        let text = std::fs::read_to_string(&path).expect("--markets-json file");
        let value: serde_json::Value = serde_json::from_str(&text).expect("--markets-json JSON");
        let _ = MARKETS_OVERRIDE.set(value);
    }
}

pub(crate) fn apply_markets_override(session: &mut Session) -> Result<(), String> {
    let Some(overlay) = MARKETS_OVERRIDE.get() else {
        return Ok(());
    };
    let mut rules = serde_json::to_value(&session.dataset.markets).map_err(|e| e.to_string())?;
    if let (Some(rules), Some(overlay)) = (rules.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay {
            if !rules.contains_key(key) {
                return Err(format!("--markets-json: unknown market rule {key}"));
            }
            rules.insert(key.clone(), value.clone());
        }
    }
    session.dataset.markets = serde_json::from_value(rules).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn run(arguments: &[String]) {
    VERBOSE.store(
        arguments.iter().any(|a| a == "--verbose"),
        std::sync::atomic::Ordering::Relaxed,
    );
    let seeds: u64 = flag(arguments, "--seeds")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let from: u64 = flag(arguments, "--from")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|v| v.parse().ok())
        .unwrap_or(520);
    let scenario_id = flag(arguments, "--scenario-id").unwrap_or_else(|| "great-lakes-2000".into());
    let difficulty_key = flag(arguments, "--difficulty").unwrap_or_else(|| "standard".into());
    load_markets_override(arguments);
    let difficulty = cw_data::Difficulty::from_key(&difficulty_key);
    let policy = crate::policy_flag(arguments);
    let gates_json = match flag(arguments, "--gates") {
        Some(path) => fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("cannot read {path}: {e}");
            std::process::exit(1);
        }),
        None => DEFAULT_GATES.to_owned(),
    };
    // A prior `--policy none` JSON for the same scenario, seeds and weeks avoids replaying none.
    let none_cache: BTreeMap<u64, Vec<(u64, bool, i64)>> = flag(arguments, "--none-json")
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<ArcReport>(&text).ok())
        .map(|r| {
            r.careers
                .into_iter()
                .filter(|c| c.weeks_played == weeks)
                .map(|c| (c.seed, c.final_firms))
                .collect()
        })
        .unwrap_or_default();

    // A prior `--policy noise` JSON gives each seed's floor against the same none run.
    let noise_cache: BTreeMap<u64, Vec<(u64, bool, i64)>> = flag(arguments, "--noise-json")
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<ArcReport>(&text).ok())
        .map(|r| {
            r.careers
                .into_iter()
                .filter(|c| c.weeks_played == weeks)
                .map(|c| (c.seed, c.final_firms))
                .collect()
        })
        .unwrap_or_default();

    let mut careers = Vec::new();
    let mut errors = Vec::new();
    let mut check = ScenarioCheck::default();
    for seed in from..from + seeds {
        match play(seed, weeks, &scenario_id, difficulty, policy) {
            Ok((mut career, scenario)) => {
                check = scenario;
                if policy != ReferencePolicy::None {
                    let none = match none_cache.get(&seed) {
                        Some(firms) => Some(firms.clone()),
                        None => play_none(seed, weeks, &scenario_id, difficulty)
                            .map_err(|e| errors.push(e))
                            .ok(),
                    };
                    career.influence = none.as_ref().map(|n| compare(n, &career.final_firms));
                    if !policy.is_noise()
                        && let (Some(n), Some(noise)) = (none.as_ref(), noise_cache.get(&seed))
                    {
                        career.influence_floor = Some(compare(n, noise));
                    }
                }
                println!(
                    "seed={} weeks={} rank={:?}/{} bust={} lost={} nw={} leader_changes={} entries={} exits={} hostile={} ms/wk={:.1}",
                    career.seed,
                    career.weeks_played,
                    career.final_rank,
                    career.final_league_size,
                    career.bust,
                    career.lost_control,
                    career.final_net_worth_cents,
                    career.league.leader_changes_held_13w,
                    career.turnover.entries,
                    career.turnover.exits,
                    career.hostile.total,
                    career.ms_per_week
                );
                careers.push(career);
            }
            Err(error) => {
                eprintln!("{error}");
                errors.push(error);
            }
        }
    }

    let aggregate = aggregate(&careers);
    let gates = evaluate_gates(&gates_json, policy.key(), &aggregate).unwrap_or_else(|e| {
        eprintln!("{e}");
        Vec::new()
    });
    let report = ArcReport {
        schema: "cw-arc/1".to_owned(),
        engine_version: cw_core::ENGINE_VERSION.to_owned(),
        rules_version: cw_core::RULES_VERSION,
        scenario: check,
        policy: policy.key().to_owned(),
        difficulty: difficulty_key,
        weeks,
        seed_from: from,
        seed_count: seeds,
        definitions: definitions(),
        careers,
        aggregate,
        gates,
        errors,
    };
    if let Some(path) = flag(arguments, "--json") {
        let text = serde_json::to_string_pretty(&report).expect("arc report serialises");
        if let Err(error) = fs::write(&path, format!("{text}\n")) {
            eprintln!("cannot write {path}: {error}");
            std::process::exit(1);
        }
        println!("wrote {path}");
    }
    let a = &report.aggregate;
    println!(
        "arc scenario={} loaded={} policy={} seeds={} weeks={} companies={} bust={:?} first={:?} bottom_half={:?} zombie={:?} price_range_pct={:?} traded={:?} local={:?} ms/wk={:.1}",
        report.scenario.requested,
        report.scenario.loaded,
        report.policy,
        a.careers,
        report.weeks,
        a.companies,
        a.bust_share,
        a.first_place_share,
        a.bottom_half_share,
        a.zombie_share,
        a.price_range_pct_median,
        a.price_range_traded_pct_median,
        a.price_range_local_pct_median,
        a.ms_per_week_mean
    );
    for gate in &report.gates {
        println!(
            "gate {:<22} {:<5} value={:?}  {}",
            gate.id, gate.result, gate.value, gate.text
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians_and_quantiles() {
        assert_eq!(median_i64(&[]), None);
        assert_eq!(median_i64(&[5, 1, 3]), Some(3));
        assert_eq!(median_i64(&[4, 1, 3, 2]), Some(2));
        assert_eq!(median_f64(&[1.0, 4.0]), Some(2.5));
        assert_eq!(
            quantile_i64(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 9, 10),
            Some(9)
        );
    }

    #[test]
    fn leader_changes_need_a_held_lead() {
        let a = Some(1);
        let b = Some(2);
        // A A B A A B B B B: raw flips A->B, B->A, A->B = 3; with a 3 week hold only the final run counts once.
        let series = [a, a, b, a, a, b, b, b, b];
        assert_eq!(leader_changes(&series, 1), 3);
        assert_eq!(leader_changes(&series, 3), 1);
        assert_eq!(leader_changes(&[a, a, a], 1), 0);
        assert_eq!(leader_changes(&[], 1), 0);
        // A gap week does not reset or change the champion.
        assert_eq!(leader_changes(&[a, None, a, b], 1), 1);
    }

    #[test]
    fn rank_is_by_size_then_handle() {
        let rows = [(7, 50), (3, 90), (9, 90), (1, 10)];
        assert_eq!(rank_of(&rows, 3), Some(1));
        assert_eq!(rank_of(&rows, 9), Some(2));
        assert_eq!(rank_of(&rows, 7), Some(3));
        assert_eq!(rank_of(&rows, 1), Some(4));
        assert_eq!(rank_of(&rows, 99), None);
    }

    #[test]
    fn bottom_half_is_strictly_past_the_middle() {
        assert!(!bottom_half(2, 4));
        assert!(bottom_half(3, 4));
        assert!(!bottom_half(2, 5));
        assert!(bottom_half(3, 5));
        assert!(bottom_half(1, 1), "a lone firm is past its own middle");
    }

    #[test]
    fn price_range_and_shares() {
        // 80 to 130 on a reference of 100 is a 50 percent range.
        assert_eq!(range_pct_x100(80, 130, 100), Some(5_000));
        assert_eq!(range_pct_x100(80, 130, 0), None);
        assert_eq!(share_x100(1, 4), 2_500);
        assert_eq!(share_x100(3, 0), 0);
        assert_eq!(per_decade(3, 260), 6.0);
    }

    #[test]
    fn difference_rules() {
        assert_eq!(differs((true, 100), (false, 100)), (true, false));
        assert_eq!(differs((false, 5), (false, 900)), (false, false));
        assert_eq!(differs((true, 100), (true, 109)), (false, false));
        assert_eq!(differs((true, 100), (true, 120)), (false, true));
        let none = [(1, true, 100), (2, true, 100), (3, false, 0)];
        let policy = [(1, true, 100), (2, true, 200), (3, true, 50)];
        let i = compare(&none, &policy);
        assert_eq!((i.compared, i.alive_differs, i.size_differs), (3, 1, 1));
    }

    fn career(rank: Option<u32>, league: u32, bust: bool) -> Career {
        Career {
            weeks_played: 520,
            had_company: true,
            final_rank: rank,
            final_league_size: league,
            bust,
            ..Career::default()
        }
    }

    #[test]
    fn aggregate_shares_and_gates() {
        let careers = vec![
            career(Some(9), 10, false),
            career(Some(1), 10, false),
            career(None, 10, true),
            career(Some(8), 10, false),
        ];
        let a = aggregate(&careers);
        assert_eq!(a.companies, 4);
        // rank 9, no rank (dead) and rank 8 are bottom half: 3 of 4.
        assert_eq!(a.bottom_half_share, Some(0.75));
        assert_eq!(a.bust_share, Some(0.25));
        assert_eq!(a.first_place_share, Some(0.25));
        let gates = evaluate_gates(DEFAULT_GATES, "passive", &a).unwrap();
        let by_id = |id: &str| gates.iter().find(|g| g.id == id).unwrap().result.clone();
        assert_eq!(by_id("passive_bottom_half"), "pass");
        assert_eq!(by_id("reckless_bust"), "n/a");
        assert_eq!(by_id("expander_first"), "n/a");
        // No firm-weeks were recorded, so the zombie gate has no data.
        assert_eq!(by_id("zombies"), "n/a");
        let reckless = evaluate_gates(DEFAULT_GATES, "reckless", &a).unwrap();
        assert_eq!(
            reckless
                .iter()
                .find(|g| g.id == "reckless_bust")
                .unwrap()
                .result,
            "pass"
        );
    }

    #[test]
    fn gate_bounds_are_inclusive_except_the_zombie_cap() {
        let mut a = Aggregate {
            zombie_share: Some(0.01),
            exits_per_decade_median: Some(4.0),
            ..Aggregate::default()
        };
        let gates = evaluate_gates(DEFAULT_GATES, "none", &a).unwrap();
        let by_id = |id: &str| gates.iter().find(|g| g.id == id).unwrap().result.clone();
        assert_eq!(by_id("zombies"), "fail");
        assert_eq!(by_id("exits"), "pass");
        a.zombie_share = Some(0.0099);
        let gates = evaluate_gates(DEFAULT_GATES, "none", &a).unwrap();
        assert_eq!(
            gates.iter().find(|g| g.id == "zombies").unwrap().result,
            "pass"
        );
    }

    #[test]
    fn same_seed_twice_gives_the_same_report() {
        let run_once = || {
            let (career, _) = play(
                3,
                30,
                "great-lakes-2000",
                cw_data::Difficulty::Standard,
                ReferencePolicy::Passive,
            )
            .unwrap();
            let mut report = ArcReport {
                aggregate: aggregate(std::slice::from_ref(&career)),
                careers: vec![career],
                ..ArcReport::default()
            };
            strip_timing(&mut report);
            serde_json::to_string(&report).unwrap()
        };
        assert_eq!(run_once(), run_once());
    }
}
