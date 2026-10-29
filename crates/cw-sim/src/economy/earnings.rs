//! Earnings season (1.9): analyst estimates, results day and management guidance.
//!
//! Every listed company (operating, shares in issue, at least one percent of them held outside)
//! reports once a quarter, on the quarter's close (weeks 13, 26, 39 and so on). Straight after a
//! results day the analyst writes an estimate of the next quarter's operating profit from the
//! firm's own trend, how its sector's listed firms are trending, and a little noise from the
//! world seed stream, so an estimate can miss in either direction.
//!
//! On results day the quarter's actual operating profit (EBIT of the thirteen closed weeks) is
//! set against the estimate. The surprise, scaled by its size against the estimate and by how
//! many quarters in a row the firm has surprised the same way, moves the share mark by a bounded
//! amount, on top of the usual price formation. A move changes the mark only: no cash, share or
//! account moves, so no money is created or destroyed (`MOVE_CAP_BPS` is the hard bound).
//!
//! Management can give guidance (`GiveGuidance`): a range for the quarter's operating profit.
//! The analyst pulls the estimate toward the middle of the range, more as the firm's credibility
//! grows, but never more than `GUIDE_PULL_CAP_BPS` of the estimate's scale. Meeting the range
//! (actual at or above its low end) builds credibility, which damps the price move on a later
//! miss. Missing it halves credibility, costs reputation and hands the activists a grievance
//! (`structure.rs`, `Reason::MissedGuidance`). A range set well below the analyst's own number
//! earns no credibility: it is sandbagging and the analyst knows it.
//!
//! Ratings (buy, hold, sell) come from the valuation against the price. A sell with the price
//! 60 percent or more above value draws an activist short campaign (`Reason::Overvalued`), which
//! uses the ordinary campaign path.
//!
//! The state lives outside the postcard payload (`World.earnings`, save trailer `EARN`) and is
//! folded into the canonical hash while it is not empty.

use std::collections::BTreeMap;

use cw_core::error::{DomainError, DomainResult};
use cw_core::ids::Handle;
use cw_core::rng::{Rng, TAG_EARNINGS};
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::state::{HolderId, World};

/// Weeks in a reporting quarter.
pub const QUARTER: u32 = 13;
/// The largest price move on a results day, in basis points either way.
pub const MOVE_CAP_BPS: i64 = 1_500;
/// Price move at a 100 percent surprise before streaks and credibility, percent of the price.
const MOVE_PER_FULL_SURPRISE_PCT: i64 = 15;
/// A surprise smaller than this (basis points of the estimate's scale) counts as in line.
const IN_LINE_BPS: i64 = 200;
/// Extra weight per quarter in a row of the same surprise, up to `STREAK_MAX` quarters.
const STREAK_STEP_BPS: i64 = 1_500;
const STREAK_MAX: i8 = 4;
/// New firms start with some credibility, in basis points of the 10,000 ceiling.
pub const CREDIBILITY_START_BPS: i32 = 3_000;
const CREDIBILITY_GAIN_BPS: i32 = 1_500;
/// Credibility keeps this share of itself after a missed range.
const CREDIBILITY_KEPT_ON_MISS_PCT: i32 = 50;
/// At full credibility a miss moves the price this much less, basis points.
const MISS_DAMP_AT_FULL_BPS: i64 = 4_000;
/// The estimate follows guidance by this much (base plus credibility share), basis points.
const GUIDE_PULL_BASE_BPS: i64 = 2_000;
const GUIDE_PULL_CAP_BPS: i64 = 1_500;
/// Guidance more than this far under the analyst's own number earns no credibility.
const SANDBAG_BPS: i64 = 1_000;
/// Widest range the analyst accepts around its own number, basis points of the scale.
const RANGE_LIMIT_BPS: i64 = 5_000;
/// Reputation lost by a missed range: base plus shortfall share, in basis points of reputation.
const MISS_REPUTATION_BASE_BPS: i32 = 150;
const MISS_REPUTATION_MAX_BPS: i32 = 400;
/// How far noise can move an estimate, basis points of its scale.
const NOISE_BPS: i64 = 1_000;
/// Buy when value is this far over the price, sell when the price is this far over value
/// (value over price in basis points).
pub const BUY_AT_BPS: i64 = 12_000;
pub const SELL_AT_BPS: i64 = 8_333;
/// A sell this deep draws a short campaign (value over price, basis points).
pub const SHORT_AT_BPS: i64 = 6_250;
/// Reports kept in the log, all firms, oldest dropped first.
const LOG_KEPT: usize = 160;
/// Quarters of operating profit kept per firm.
const QUARTERS_KEPT: usize = 5;
/// A firm needs this much of its shares held outside to be covered, basis points.
const FLOAT_MIN_BPS: i64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rating {
    Buy,
    Hold,
    Sell,
}

impl Rating {
    pub const fn key(self) -> &'static str {
        match self {
            Rating::Buy => "buy",
            Rating::Hold => "hold",
            Rating::Sell => "sell",
        }
    }
}

/// The rating for a fair value and a price, both in the numeraire.
pub fn rating_for(fair_cents: i64, price_cents: i64) -> Rating {
    if price_cents <= 0 {
        return Rating::Hold;
    }
    let ratio = (fair_cents.max(0) as i128 * 10_000 / price_cents as i128) as i64;
    if ratio >= BUY_AT_BPS {
        Rating::Buy
    } else if ratio <= SELL_AT_BPS {
        Rating::Sell
    } else {
        Rating::Hold
    }
}

/// The analyst's number for one quarter.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Estimate {
    pub made: u32,
    /// The results day this estimate is for.
    pub due: u32,
    /// Expected operating profit for the quarter, after any guidance pull.
    pub cents: i64,
    /// The analyst's own number before guidance.
    pub raw_cents: i64,
    /// Size of the number's room for error: the larger of its size and a small share of value.
    pub scale_cents: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guidance {
    pub given: u32,
    pub due: u32,
    pub low_cents: i64,
    pub high_cents: i64,
    /// Counted for credibility if met (not sandbagged).
    pub honest: bool,
}

/// One firm's results day.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub firm: Handle,
    pub week: u32,
    pub estimate_cents: i64,
    pub actual_cents: i64,
    /// Actual against estimate, basis points of the estimate's scale, bounded to +-10,000.
    pub surprise_bps: i32,
    /// The price move the surprise caused, basis points.
    pub move_bps: i32,
    pub guided: Option<(i64, i64)>,
    pub guidance_met: Option<bool>,
    /// Quarters in a row of the same surprise after this one (negative for misses).
    pub streak: i8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmEarnings {
    pub estimate: Option<Estimate>,
    pub guidance: Option<Guidance>,
    pub credibility_bps: i32,
    pub streak: i8,
    /// Operating profit of the last quarters, newest last.
    pub quarters: Vec<i64>,
    /// The week a guidance range was last missed, and how many in a row.
    pub missed_week: u32,
    pub missed_run: u8,
    pub last: Option<Report>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EarningsState {
    pub firms: BTreeMap<Handle, FirmEarnings>,
    pub log: Vec<Report>,
}

impl EarningsState {
    pub fn is_empty(&self) -> bool {
        *self == EarningsState::default()
    }

    /// Justification (basis points) for an activist grievance over a missed range in the last
    /// quarter, else none.
    pub fn guidance_grievance(&self, firm: Handle, tick: u32) -> Option<i32> {
        let fe = self.firms.get(&firm)?;
        (fe.missed_run > 0 && tick.saturating_sub(fe.missed_week) <= QUARTER)
            .then(|| (5_000 + 1_500 * fe.missed_run as i32).min(9_000))
    }

    /// Weeks until the firm's next results day, if it has an estimate outstanding.
    pub fn weeks_to_results(&self, firm: Handle, tick: u32) -> Option<u32> {
        let e = self.firms.get(&firm)?.estimate.as_ref()?;
        (e.due >= tick).then(|| e.due - tick)
    }

    pub fn credibility(&self, firm: Handle) -> i32 {
        self.firms
            .get(&firm)
            .map(|f| f.credibility_bps)
            .unwrap_or(CREDIBILITY_START_BPS)
    }
}

/// A firm the analysts cover: operating, on the board and floated.
pub fn covered(world: &World, firm: Handle) -> bool {
    let Some(f) = world.firms.get(firm) else {
        return false;
    };
    f.on_board() && f.state_owned_since.is_none() && float_bps(world, firm) >= FLOAT_MIN_BPS
}

fn float_bps(world: &World, firm: Handle) -> i64 {
    world
        .firms
        .get(firm)
        .map(|f| {
            let outside: i64 = f
                .shares
                .holders
                .iter()
                .filter(|(h, _)| matches!(h, HolderId::Outside(_)))
                .map(|(_, lot)| lot.qty)
                .sum();
            outside * 10_000 / f.shares.issued.max(1)
        })
        .unwrap_or(0)
}

fn market_cap(world: &World, firm: Handle) -> i64 {
    world
        .firms
        .get(firm)
        .map(|f| f.shares.mark_cents.max(0).saturating_mul(f.shares.issued))
        .unwrap_or(0)
}

/// The room for error of an estimate: its size, but never under half a percent of market value.
fn scale_of(world: &World, firm: Handle, base: i64) -> i64 {
    base.abs().max(market_cap(world, firm) / 200).max(1)
}

/// Operating profit of the thirteen closed weeks ending this week, or none when the firm has
/// not closed a full quarter.
fn quarter_ebit(world: &World, firm: Handle) -> Option<i64> {
    let closes = world.firms.get(firm)?.book.recent_weeks();
    let tick = world.tick.0;
    let window: Vec<_> = closes
        .iter()
        .filter(|w| w.week.0 + QUARTER > tick && w.week.0 <= tick)
        .collect();
    (window.len() as u32 == QUARTER).then(|| window.iter().map(|w| w.ebit()).sum())
}

/// The weekly step. Runs after the week's books close; only does work on a quarter's close.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    let tick = world.tick.0;
    let alive: Vec<Handle> = world.firms.iter().map(|(h, _)| h).collect();
    world.earnings.firms.retain(|h, _| alive.contains(h));
    if tick == 0 || tick % QUARTER != 0 {
        return;
    }
    let covered_firms: Vec<Handle> = alive
        .iter()
        .copied()
        .filter(|h| covered(world, *h))
        .collect();
    // Close the quarter for every covered firm, in handle order.
    let mut actuals: BTreeMap<Handle, i64> = BTreeMap::new();
    for firm in &covered_firms {
        if let Some(actual) = quarter_ebit(world, *firm) {
            actuals.insert(*firm, actual);
        }
    }
    for firm in &covered_firms {
        if let Some(actual) = actuals.get(firm) {
            report(world, *firm, *actual);
        }
    }
    // Next quarter's estimates, after every firm's new actual is on record.
    let sector_trend = sector_trends(world, &covered_firms);
    for firm in &covered_firms {
        if actuals.contains_key(firm) {
            estimate(world, dataset, *firm, &sector_trend);
        }
    }
    // Firms that dropped out of coverage keep nothing outstanding.
    for (handle, fe) in world.earnings.firms.iter_mut() {
        if !covered_firms.contains(handle) {
            fe.estimate = None;
            fe.guidance = None;
        }
    }
}

/// Results day for one firm: set the actual against the estimate and move the mark.
fn report(world: &mut World, firm: Handle, actual: i64) {
    let tick = world.tick.0;
    let fe = world
        .earnings
        .firms
        .entry(firm)
        .or_insert_with(|| FirmEarnings {
            credibility_bps: CREDIBILITY_START_BPS,
            ..FirmEarnings::default()
        });
    fe.quarters.push(actual);
    if fe.quarters.len() > QUARTERS_KEPT {
        fe.quarters.remove(0);
    }
    let estimate = match fe.estimate.take() {
        Some(e) if e.due == tick => e,
        _ => {
            fe.guidance = None;
            return;
        }
    };
    let guidance = fe.guidance.take().filter(|g| g.due == tick);
    let rel = ((actual as i128 - estimate.cents as i128) * 10_000
        / estimate.scale_cents.max(1) as i128)
        .clamp(-10_000, 10_000) as i64;
    // Streak of the same surprise.
    let old_streak = fe.streak;
    let new_streak: i8 = if rel > IN_LINE_BPS {
        old_streak.max(0).saturating_add(1).min(8)
    } else if rel < -IN_LINE_BPS {
        old_streak.min(0).saturating_sub(1).max(-8)
    } else {
        0
    };
    let same_direction = (rel > 0 && old_streak > 0) || (rel < 0 && old_streak < 0);
    let run = if same_direction {
        old_streak.abs().min(STREAK_MAX) as i64
    } else {
        0
    };
    let mut weight = 10_000 + run * STREAK_STEP_BPS;
    if rel < 0 {
        let damp = fe.credibility_bps as i64 * MISS_DAMP_AT_FULL_BPS / 10_000;
        weight = weight * (10_000 - damp) / 10_000;
    }
    let mv = (rel * MOVE_PER_FULL_SURPRISE_PCT / 100 * weight / 10_000)
        .clamp(-MOVE_CAP_BPS, MOVE_CAP_BPS);

    // Guidance outcome.
    let mut guidance_met = None;
    let mut reputation_delta = 0;
    if let Some(g) = &guidance {
        let met = actual >= g.low_cents;
        guidance_met = Some(met);
        if met {
            if g.honest {
                fe.credibility_bps = (fe.credibility_bps + CREDIBILITY_GAIN_BPS).min(10_000);
            }
            fe.missed_run = 0;
        } else {
            fe.credibility_bps = fe.credibility_bps * CREDIBILITY_KEPT_ON_MISS_PCT / 100;
            fe.missed_run = fe.missed_run.saturating_add(1);
            fe.missed_week = tick;
            let short = ((g.low_cents as i128 - actual as i128) * 10_000
                / estimate.scale_cents.max(1) as i128)
                .clamp(0, 10_000) as i32;
            reputation_delta =
                -(MISS_REPUTATION_BASE_BPS + short / 40).min(MISS_REPUTATION_MAX_BPS);
        }
    }
    fe.streak = new_streak;
    let record = Report {
        firm,
        week: tick,
        estimate_cents: estimate.cents,
        actual_cents: actual,
        surprise_bps: rel as i32,
        move_bps: mv as i32,
        guided: guidance.as_ref().map(|g| (g.low_cents, g.high_cents)),
        guidance_met,
        streak: new_streak,
    };
    fe.last = Some(record.clone());
    world.earnings.log.push(record);
    let excess = world.earnings.log.len().saturating_sub(LOG_KEPT);
    world.earnings.log.drain(..excess);

    // Prices only: the mark moves, nothing else.
    if let Some(f) = world.firms.get_mut(firm) {
        let mark = f.shares.mark_cents.max(0);
        let moved = (mark as i128 * (10_000 + mv) as i128 / 10_000) as i64;
        f.shares.mark_cents = if mark > 0 { moved.max(1) } else { mark };
        if reputation_delta != 0 {
            let rewards = &cw_data::chapters::mandate_table().rewards;
            let before = f.reputation_bps;
            f.reputation_bps = (before + reputation_delta).clamp(
                rewards.reputation_floor_bps.min(before),
                rewards.reputation_ceiling_bps.max(before),
            );
        }
    }
}

/// Mean quarter on quarter change of listed firms by industry, basis points (bounded).
fn sector_trends(world: &World, firms: &[Handle]) -> BTreeMap<u8, i64> {
    let mut sums: BTreeMap<u8, (i64, i64)> = BTreeMap::new();
    for firm in firms {
        let Some(fe) = world.earnings.firms.get(firm) else {
            continue;
        };
        let n = fe.quarters.len();
        if n < 2 {
            continue;
        }
        let (last, prev) = (fe.quarters[n - 1], fe.quarters[n - 2]);
        let scale = scale_of(world, *firm, prev);
        let change =
            ((last as i128 - prev as i128) * 10_000 / scale as i128).clamp(-3_000, 3_000) as i64;
        let industry = world.firms.get(*firm).map(|f| f.industry).unwrap_or(0);
        let e = sums.entry(industry).or_default();
        e.0 += change;
        e.1 += 1;
    }
    sums.into_iter()
        .map(|(k, (s, n))| (k, s / n.max(1)))
        .collect()
}

/// Write the analyst's estimate for the quarter that starts now.
fn estimate(world: &mut World, _dataset: &Dataset, firm: Handle, sector: &BTreeMap<u8, i64>) {
    let tick = world.tick.0;
    let Some(fe) = world.earnings.firms.get(&firm) else {
        return;
    };
    let n = fe.quarters.len();
    let Some(&last) = fe.quarters.last() else {
        return;
    };
    let scale = scale_of(world, firm, last);
    let trend = if n >= 2 {
        let prev = fe.quarters[n - 2];
        ((last - prev) / 2).clamp(-scale / 4, scale / 4)
    } else {
        0
    };
    let industry = world.firms.get(firm).map(|f| f.industry).unwrap_or(0);
    let sector_bps = sector.get(&industry).copied().unwrap_or(0) / 4;
    let sector_part = (scale as i128 * sector_bps.clamp(-1_000, 1_000) as i128 / 10_000) as i64;
    let mut rng = Rng::entity_stream(
        world.seed,
        TAG_EARNINGS,
        Handle(firm.0 ^ ((tick as u64) << 40)),
    );
    let noise = rng.uniform_inclusive(-NOISE_BPS, NOISE_BPS);
    let noise_part = (scale as i128 * noise as i128 / 10_000) as i64;
    let raw = last + trend + sector_part + noise_part;
    let entry = world.earnings.firms.entry(firm).or_default();
    entry.estimate = Some(Estimate {
        made: tick,
        due: tick + QUARTER,
        cents: raw,
        raw_cents: raw,
        scale_cents: scale,
    });
    entry.guidance = None;
}

/// `GiveGuidance`: state a range for this quarter's operating profit.
pub fn give_guidance(world: &mut World, firm: Handle, low: i64, high: i64) -> DomainResult<()> {
    let tick = world.tick.0;
    if !covered(world, firm) {
        return Err(DomainError::WrongPhase {
            expected: "a listed company",
            actual: "no analysts cover this company",
        });
    }
    if low > high {
        return Err(DomainError::WrongPhase {
            expected: "a range with the low end under the high end",
            actual: "low end above high end",
        });
    }
    let credibility;
    let (raw, scale, due) = {
        let fe = world.earnings.firms.get(&firm);
        let Some(est) = fe.and_then(|f| f.estimate.as_ref()) else {
            return Err(DomainError::WrongPhase {
                expected: "an analyst estimate for this quarter",
                actual: "no estimate yet",
            });
        };
        if est.due <= tick {
            return Err(DomainError::WrongPhase {
                expected: "guidance before results day",
                actual: "results are due",
            });
        }
        if fe.is_some_and(|f| f.guidance.is_some()) {
            return Err(DomainError::WrongPhase {
                expected: "one range per quarter",
                actual: "guidance already given",
            });
        }
        credibility = fe
            .map(|f| f.credibility_bps)
            .unwrap_or(CREDIBILITY_START_BPS);
        (est.raw_cents, est.scale_cents, est.due)
    };
    let limit = scale * RANGE_LIMIT_BPS / 10_000;
    if low < raw - limit || high > raw + limit {
        return Err(DomainError::WrongPhase {
            expected: "a range within half the analyst's estimate",
            actual: "range too far from the estimate",
        });
    }
    let honest = low >= raw - scale * SANDBAG_BPS / 10_000;
    let mid = (low + high) / 2;
    let pull_bps = (GUIDE_PULL_BASE_BPS + credibility as i64 * 6 / 10).min(8_000);
    let pull_cap = scale * GUIDE_PULL_CAP_BPS / 10_000;
    let shift = ((mid - raw) as i128 * pull_bps as i128 / 10_000) as i64;
    let shift = shift.clamp(-pull_cap, pull_cap);
    if let Some(fe) = world.earnings.firms.get_mut(&firm) {
        if let Some(est) = fe.estimate.as_mut() {
            est.cents = raw + shift;
        }
        fe.guidance = Some(Guidance {
            given: tick,
            due,
            low_cents: low,
            high_cents: high,
            honest,
        });
    }
    Ok(())
}

/// A rival's guidance for this quarter, if it gives one: the week after the estimate is
/// written, every second listed firm in turn, a tight range around the analyst's number when
/// profit is not falling. Rivals use the same command and the same checks as the player.
pub fn rival_guidance(world: &World, firm: Handle) -> Option<(i64, i64)> {
    if !covered(world, firm) {
        return None;
    }
    let fe = world.earnings.firms.get(&firm)?;
    let est = fe.estimate.as_ref()?;
    if fe.guidance.is_some() || world.tick.0 != est.made + 1 {
        return None;
    }
    if (firm.slot() as u32 + est.due / QUARTER) % 2 != 0 {
        return None;
    }
    let n = fe.quarters.len();
    if n >= 2 && fe.quarters[n - 1] < fe.quarters[n - 2] {
        return None;
    }
    let room = est.scale_cents * 500 / 10_000;
    Some((est.raw_cents - room, est.raw_cents + room))
}

/// The price a short campaign knocks off when it opens, basis points, bounded.
pub fn short_hit(world: &mut World, firm: Handle, justification_bps: i32) {
    let hit = (300 + justification_bps as i64 / 20).min(800);
    if let Some(f) = world.firms.get_mut(firm) {
        let mark = f.shares.mark_cents.max(0);
        if mark > 0 {
            f.shares.mark_cents = (mark as i128 * (10_000 - hit) as i128 / 10_000).max(1) as i64;
        }
    }
}

/// Whether the firm's price is far enough over value to draw a short campaign, and how strong the
/// case is (basis points).
pub fn overvalued_case(world: &World, firm: Handle) -> Option<i32> {
    let f = world.firms.get(firm)?;
    let mark = f.shares.mark_cents;
    if mark <= 0 || f.shares.fair_cents <= 0 {
        return None;
    }
    let ratio = f.shares.fair_cents as i128 * 10_000 / mark as i128;
    (ratio <= SHORT_AT_BPS as i128).then(|| ((10_000 - ratio) * 3 / 2).clamp(0, 9_000) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratings_follow_value_against_price() {
        assert_eq!(rating_for(1_300, 1_000), Rating::Buy);
        assert_eq!(rating_for(1_200, 1_000), Rating::Buy);
        assert_eq!(rating_for(1_100, 1_000), Rating::Hold);
        assert_eq!(rating_for(1_000, 1_000), Rating::Hold);
        assert_eq!(rating_for(900, 1_000), Rating::Hold);
        assert_eq!(rating_for(833, 1_000), Rating::Sell);
        assert_eq!(rating_for(500, 1_000), Rating::Sell);
        assert_eq!(rating_for(0, 1_000), Rating::Sell);
        assert_eq!(rating_for(500, 0), Rating::Hold);
    }
}
