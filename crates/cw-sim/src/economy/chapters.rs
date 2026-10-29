//! Chapters and board mandates (1.9).
//!
//! Every 52 weeks of a career is a chapter. In its first week the board of each operating company
//! offers three mandates, drawn from facts the engine already keeps; the company picks one with
//! the `ChooseMandate` command inside the offer window (four weeks) or the offer lapses with no
//! penalty. The player's board is the primary company's; every rival's board offers its own, and
//! rivals pick through the same command (`cw-ai`), by the same rules. At the end of the chapter a
//! met mandate pays the management bonus (player side only; a rival has no personal purse) and
//! reputation, a missed one costs reputation, and two missed in a row in the player's company
//! draw an activist (`structure.rs`, Reason::MissedGoals).
//!
//! This replaces the 1.2 quarterly board objectives (`objectives.rs`), which survive only as the
//! saved layout of the old fields. The numbers are in `content/mandates.json`.
//!
//! The state lives outside the postcard payload (`World.chapters`, save trailer `CHAP`) and is
//! folded into the canonical hash, so a reload mid-chapter plays on exactly as an unbroken game.
//! Nothing here draws a random number.

use std::collections::{BTreeMap, BTreeSet};

use cw_core::error::{ArithmeticFault, DomainError, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::chapters::{self as table, MandateTable};
use cw_data::{Dataset, Difficulty};
use serde::{Deserialize, Serialize};

use crate::economy::{achievements, ownership, sectors};
use crate::state::{FacilityStatus, World};

pub const CHAPTER_WEEKS: u32 = 52;
/// Results of the player's chapters kept in the career.
const LOG_KEPT: usize = 60;
/// Weeks of cost history a trailing average reads.
const TRAILING: usize = 13;

/// The first week of chapter `number` (chapters count from 1).
pub fn chapter_start(number: u32) -> u32 {
    CHAPTER_WEEKS * number.saturating_sub(1) + 1
}

/// The chapter a week falls in. The opening week (tick 0) is in chapter 1.
pub fn chapter_of(tick: u32) -> u32 {
    tick.saturating_sub(1) / CHAPTER_WEEKS + 1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MandateKind {
    /// At the end of the chapter, hold at least `target_bps` of a division's market.
    MarketShare {
        region: u8,
        industry: u8,
        target_bps: i64,
    },
    /// Own an operating plant that makes `product`, an input the company buys today.
    Integrate { product: u16 },
    /// A subsidiary of the group lists on the exchange (more listed members than at the start).
    ListSubsidiary { baseline: u32 },
    /// Keep cash at or above `cover_weeks` of the company's weekly costs, every week.
    CashCover { cover_weeks: u32 },
    /// Win a raid on a rival's plants.
    WinRaid,
    /// Average weekly operating profit over the last 13 weeks at or above the target at the end.
    ProfitGrowth {
        baseline_cents: i64,
        target_cents: i64,
    },
    /// Debt to equity at or below the target at the end of the chapter.
    DebtToEquity { target_bps: i64 },
}

impl MandateKind {
    pub const fn key(&self) -> &'static str {
        match self {
            MandateKind::MarketShare { .. } => "market_share",
            MandateKind::Integrate { .. } => "integrate",
            MandateKind::ListSubsidiary { .. } => "list_subsidiary",
            MandateKind::CashCover { .. } => "cash_cover",
            MandateKind::WinRaid => "win_raid",
            MandateKind::ProfitGrowth { .. } => "profit_growth",
            MandateKind::DebtToEquity { .. } => "debt_to_equity",
        }
    }
}

/// Offer rotation order. A company's first offered kind moves with the chapter and its handle.
const ORDER: usize = 7;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub chapter: u32,
    pub set: u32,
    pub kinds: Vec<MandateKind>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandate {
    pub firm: Handle,
    pub chapter: u32,
    pub set: u32,
    /// The week the chapter closes and the mandate is settled.
    pub due: u32,
    pub kind: MandateKind,
    /// Latched when an event kind has happened (integrated, listed, raid won).
    pub met: bool,
    /// Set when a cash cover week fell short.
    pub broken: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Met,
    Missed,
    /// The offer was not taken inside the window.
    Lapsed,
    /// The company stopped operating before the chapter closed.
    Cancelled,
}

/// One chapter of the player's career.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterLog {
    pub number: u32,
    pub name: String,
    pub start: u32,
    pub firm: Option<Handle>,
    pub offered: Vec<MandateKind>,
    pub chosen: Option<u8>,
    pub outcome: Option<Outcome>,
    pub closed: u32,
    pub bonus_cents: i64,
    pub reputation_bps: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Earned {
    pub id: String,
    pub tick: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterState {
    /// The last chapter opened.
    pub chapter: u32,
    pub offers: BTreeMap<Handle, Offer>,
    pub active: BTreeMap<Handle, Mandate>,
    /// Rivals' mandates met and missed, for their conduct record.
    pub records: BTreeMap<Handle, (u16, u16)>,
    /// The player's chapters, oldest first.
    pub log: Vec<ChapterLog>,
    pub achievements: Vec<Earned>,
}

impl ChapterState {
    pub fn is_empty(&self) -> bool {
        *self == ChapterState::default()
    }

    /// The player's mandates met and missed in a row at the end, newest last.
    pub fn results(&self) -> impl Iterator<Item = Outcome> + '_ {
        self.log.iter().filter_map(|c| c.outcome)
    }
}

// ----- facts -----

fn blocks_of(world: &World, firm: Handle) -> u64 {
    world
        .firms
        .get(firm)
        .map(|f| {
            f.facilities
                .iter()
                .filter_map(|h| world.facilities.get(*h))
                .map(|p| p.blocks as u64)
                .sum()
        })
        .unwrap_or(0)
}

/// Average weekly costs (goods, wages, running costs) over the trailing weeks the books keep.
fn weekly_costs(world: &World, firm: Handle, min_weeks: usize) -> Option<i64> {
    let closes = world.firms.get(firm)?.book.recent_weeks();
    if closes.len() < min_weeks.max(1) {
        return None;
    }
    let take = closes.len().min(TRAILING);
    let slice = &closes[closes.len() - take..];
    let total: i64 = slice
        .iter()
        .map(|w| w.cost_of_goods + w.wages + w.operating_expense)
        .sum();
    Some(total / take as i64)
}

/// Average weekly operating profit over the trailing weeks the books keep.
fn weekly_ebit(world: &World, firm: Handle, min_weeks: usize) -> Option<i64> {
    let closes = world.firms.get(firm)?.book.recent_weeks();
    if closes.len() < min_weeks.max(1) {
        return None;
    }
    let take = closes.len().min(TRAILING);
    Some(
        closes[closes.len() - take..]
            .iter()
            .map(|w| w.ebit())
            .sum::<i64>()
            / take as i64,
    )
}

fn debt_equity_bps(world: &World, firm: Handle) -> Option<i64> {
    let book = &world.firms.get(firm)?.book;
    let equity = book.total_equity();
    if equity <= 0 {
        return None;
    }
    Some((book.debt().max(0) as i128 * 10_000 / equity as i128) as i64)
}

fn listed_members(world: &World, firm: Handle) -> u32 {
    ownership::controlled_group(world, firm)
        .into_iter()
        .filter(|h| *h != firm)
        .filter(|h| world.firms.get(*h).is_some_and(|f| f.shares.ipo_done))
        .count() as u32
}

/// Product codes a firm's plants make (outputs and by-products) with `operating_only` limiting
/// to plants that are running.
fn products_made(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    operating_only: bool,
) -> BTreeSet<u16> {
    let mut made = BTreeSet::new();
    let Some(entity) = world.firms.get(firm) else {
        return made;
    };
    for plant in entity
        .facilities
        .iter()
        .filter_map(|h| world.facilities.get(*h))
    {
        if operating_only && plant.status != FacilityStatus::Operating {
            continue;
        }
        let Some(recipe) = dataset.recipes.get(plant.recipe as usize) else {
            continue;
        };
        if let Some(code) = dataset.product_index.get(&recipe.output) {
            made.insert(*code);
        }
        for by in &recipe.by_products {
            if let Some(code) = dataset.product_index.get(&by.product) {
                made.insert(*code);
            }
        }
    }
    made
}

/// The input the firm buys most of that none of its plants makes, if some recipe could make it.
fn integration_target(world: &World, dataset: &Dataset, firm: Handle) -> Option<u16> {
    let entity = world.firms.get(firm)?;
    let made = products_made(world, dataset, firm, false);
    let mut inputs: BTreeMap<u16, i64> = BTreeMap::new();
    for plant in entity
        .facilities
        .iter()
        .filter_map(|h| world.facilities.get(*h))
    {
        let recipe = dataset.recipes.get(plant.recipe as usize)?;
        for (id, lots) in &recipe.weekly_inputs_lots {
            if let Some(code) = dataset.product_index.get(id) {
                *inputs.entry(*code).or_insert(0) += lots * plant.blocks as i64;
            }
        }
    }
    let makeable = |code: u16| {
        dataset.recipes.iter().any(|r| {
            r.storage_class.is_none()
                && r.transport_mode.is_none()
                && dataset.product_index.get(&r.output) == Some(&code)
        })
    };
    inputs
        .into_iter()
        .filter(|(code, _)| !made.contains(code) && makeable(*code))
        .max_by_key(|(code, lots)| (*lots, std::cmp::Reverse(*code)))
        .map(|(code, _)| code)
}

fn largest_division(world: &World, dataset: &Dataset, firm: Handle) -> Option<sectors::Division> {
    sectors::divisions_of(world, dataset, firm)
        .into_iter()
        .max_by_key(|d| (d.blocks, std::cmp::Reverse(d.region)))
}

fn stretch(value: i64, multiplier_bps: i64) -> i64 {
    value * multiplier_bps / 10_000
}

/// The terms the board would set for one kind, or none when the company cannot be asked for it.
fn terms(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    slot: usize,
    t: &MandateTable,
    difficulty: Difficulty,
) -> Option<MandateKind> {
    let band = t.band(blocks_of(world, firm));
    let hard = t.difficulty(difficulty);
    match slot % ORDER {
        0 => {
            let division = largest_division(world, dataset, firm)?;
            let share =
                sectors::share_bps(world, dataset, firm, division.region, division.industry, 0);
            if share >= t.share_unreachable_bps {
                return None;
            }
            Some(MandateKind::MarketShare {
                region: division.region,
                industry: division.industry,
                target_bps: (share + stretch(band.share_step_bps, hard)).min(t.share_ceiling_bps),
            })
        }
        1 => {
            // A company too young to have a record is asked to turn a profit from nothing.
            let baseline = weekly_ebit(world, firm, t.min_weeks_of_history).unwrap_or(0);
            let rise = stretch(baseline.abs() * band.profit_growth_bps / 10_000, 10_000)
                .max(band.profit_min_step_cents);
            Some(MandateKind::ProfitGrowth {
                baseline_cents: baseline,
                target_cents: baseline + stretch(rise, hard),
            })
        }
        2 => integration_target(world, dataset, firm)
            .map(|product| MandateKind::Integrate { product }),
        3 => {
            let cost = weekly_costs(world, firm, t.min_weeks_of_history)?;
            let weeks = (stretch(band.cash_cover_weeks * 10_000, hard) + 9_999) / 10_000;
            let cash = world.firms.get(firm)?.book.cash();
            // Only asked when the company is already comfortably above the line.
            if cost <= 0 || cash < cost * weeks * 11 / 10 {
                return None;
            }
            Some(MandateKind::CashCover {
                cover_weeks: weeks as u32,
            })
        }
        4 => {
            let current = debt_equity_bps(world, firm)?;
            if current <= t.debt_equity_floor_bps + 500 {
                return None;
            }
            let cut = stretch(band.debt_equity_cut_bps, hard).min(9_000);
            let target = (current * (10_000 - cut) / 10_000).max(t.debt_equity_floor_bps);
            Some(MandateKind::DebtToEquity { target_bps: target })
        }
        5 => {
            largest_division(world, dataset, firm)?;
            Some(MandateKind::WinRaid)
        }
        _ => {
            let eligible = ownership::controlled_group(world, firm)
                .into_iter()
                .any(|h| {
                    h != firm
                        && world.firms.get(h).is_some_and(|f| {
                            f.operative() && !f.shares.ipo_done && f.bank.is_none()
                        })
                });
            eligible.then(|| MandateKind::ListSubsidiary {
                baseline: listed_members(world, firm),
            })
        }
    }
}

/// The three mandates a board offers a company at the start of a chapter. Pure in the world:
/// the same world offers the same three, in the same order.
pub fn offer_kinds(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    chapter: u32,
) -> Vec<MandateKind> {
    let t = table::mandate_table();
    let start = (chapter as usize * 3 + firm.slot() as usize) % ORDER;
    (0..ORDER)
        .filter_map(|step| terms(world, dataset, firm, start + step, t, world.difficulty))
        .take(3)
        .collect()
}

// ----- naming -----

/// The name of a chapter: the era of the first timeline event playing in it, else a generic one.
pub fn chapter_name(world: &World, dataset: &Dataset, number: u32) -> String {
    let names = table::chapter_names(&dataset.scenario_id);
    let (from, to) = (chapter_start(number), chapter_start(number + 1));
    let era = world
        .systemic
        .timeline
        .events
        .iter()
        .filter(|e| e.tick >= from && e.tick < to)
        .filter_map(|e| names.eras.get(&e.id).map(|name| (e.tick, name)))
        .min();
    let name = match era {
        Some((_, era)) => era.clone(),
        None => names.names[(number as usize - 1) % names.names.len()].clone(),
    };
    format!("Chapter {number}: {name}")
}

// ----- progress -----

/// How far a mandate stands, in basis points of its target, with the measured figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub bps: i64,
    /// The figure now (share bps, weekly cents, cover in tenths of a week, ratio bps, 0 or 1).
    pub now: i64,
    /// A cash cover week fell short; no longer reachable this chapter.
    pub broken: bool,
}

pub fn progress(world: &World, dataset: &Dataset, mandate: &Mandate) -> Progress {
    let firm = mandate.firm;
    let done = |met: bool| Progress {
        bps: if met { 10_000 } else { 0 },
        now: met as i64,
        broken: false,
    };
    match &mandate.kind {
        MandateKind::MarketShare {
            region,
            industry,
            target_bps,
        } => {
            let share = sectors::share_bps(world, dataset, firm, *region, *industry, 0);
            Progress {
                bps: (share * 10_000 / (*target_bps).max(1)).clamp(0, 10_000),
                now: share,
                broken: false,
            }
        }
        MandateKind::Integrate { .. }
        | MandateKind::ListSubsidiary { .. }
        | MandateKind::WinRaid => done(mandate.met),
        MandateKind::CashCover { .. } => {
            let cost = weekly_costs(world, firm, 1).unwrap_or(0);
            let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
            let tenths = if cost > 0 { cash.max(0) * 10 / cost } else { 0 };
            let span = (mandate.due - mandate.set).max(1) as i64;
            let elapsed = (world.tick.0.saturating_sub(mandate.set) as i64).min(span);
            Progress {
                bps: if mandate.broken {
                    0
                } else {
                    elapsed * 10_000 / span
                },
                now: tenths,
                broken: mandate.broken,
            }
        }
        MandateKind::ProfitGrowth {
            baseline_cents,
            target_cents,
        } => {
            let now = weekly_ebit(world, firm, 1).unwrap_or(*baseline_cents);
            let gap = (*target_cents - *baseline_cents).max(1);
            Progress {
                bps: ((now - *baseline_cents) * 10_000 / gap).clamp(0, 10_000),
                now,
                broken: false,
            }
        }
        MandateKind::DebtToEquity { target_bps } => {
            let now = debt_equity_bps(world, firm);
            let bps = match now {
                Some(ratio) if ratio <= *target_bps => 10_000,
                Some(ratio) => (*target_bps * 10_000 / ratio.max(1)).clamp(0, 9_999),
                None => 0,
            };
            Progress {
                bps,
                now: now.unwrap_or(0),
                broken: false,
            }
        }
    }
}

/// Whether the mandate counts as met if the chapter closed now.
fn succeeded(world: &World, dataset: &Dataset, mandate: &Mandate) -> bool {
    match &mandate.kind {
        MandateKind::CashCover { .. } => !mandate.broken,
        MandateKind::Integrate { .. }
        | MandateKind::ListSubsidiary { .. }
        | MandateKind::WinRaid => mandate.met,
        _ => progress(world, dataset, mandate).bps >= 10_000,
    }
}

/// The offer a rival's board takes: the mandate already furthest along, the first on a tie.
/// Pure in the world, so a reloaded game picks the same.
pub fn rival_pick(world: &World, dataset: &Dataset, firm: Handle) -> Option<u8> {
    let offer = world.chapters.offers.get(&firm)?;
    let mut best: Option<(i64, usize)> = None;
    for (index, kind) in offer.kinds.iter().enumerate() {
        let trial = Mandate {
            firm,
            chapter: offer.chapter,
            set: offer.set,
            due: chapter_start(offer.chapter + 1),
            kind: kind.clone(),
            met: false,
            broken: false,
        };
        let bps = progress(world, dataset, &trial).bps;
        if best.is_none_or(|(top, _)| bps > top) {
            best = Some((bps, index));
        }
    }
    best.map(|(_, index)| index as u8)
}

// ----- command -----

/// Validate and apply a board's pick. `actor` has already been checked to control `firm`.
pub fn choose(world: &mut World, firm: Handle, pick: u8) -> Result<(), DomainError> {
    if world.firms.get(firm).is_none_or(|f| !f.operative()) {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    }
    let Some(offer) = world.chapters.offers.get(&firm) else {
        return Err(DomainError::WrongPhase {
            expected: "mandate offer open",
            actual: "no offer",
        });
    };
    let window = table::mandate_table().offer_window_weeks;
    if world.tick.0 >= offer.set + window {
        return Err(DomainError::WrongPhase {
            expected: "mandate offer open",
            actual: "offer lapsed",
        });
    }
    let Some(kind) = offer.kinds.get(pick as usize).cloned() else {
        return Err(DomainError::Arithmetic {
            detail: ArithmeticFault::OutOfRange,
        });
    };
    let offer = world.chapters.offers.remove(&firm).expect("checked above");
    let mandate = Mandate {
        firm,
        chapter: offer.chapter,
        set: offer.set,
        due: chapter_start(offer.chapter + 1),
        kind,
        met: false,
        broken: false,
    };
    world.chapters.active.insert(firm, mandate);
    if let Some(entry) = world
        .chapters
        .log
        .iter_mut()
        .rev()
        .find(|c| c.number == offer.chapter && c.firm == Some(firm))
    {
        entry.chosen = Some(pick);
    }
    Ok(())
}

// ----- weekly -----

fn player_side(world: &World, firm: Handle) -> bool {
    ownership::ultimate_controller(world, firm) == Some(crate::state::HolderId::Investor)
}

/// A firm gets a board offer when it runs plants and is not a state firm.
fn offered_to(world: &World, firm: Handle) -> bool {
    let primary = world.investor.primary_group == Some(firm);
    world.firms.get(firm).is_some_and(|f| {
        f.operative()
            && f.state_owned_since.is_none()
            && !f.facilities.is_empty()
            && (primary || !player_side(world, firm))
    })
}

fn open_chapter(world: &mut World, dataset: &Dataset, number: u32) {
    let window = table::mandate_table().offer_window_weeks;
    let start = chapter_start(number);
    world.chapters.chapter = number;
    world.chapters.offers.clear();
    // A save from before chapters, loaded late in a chapter, opens it without an offer.
    if world.tick.0 >= start + window {
        return;
    }
    let mut handles: Vec<Handle> = world.firms.iter().map(|(h, _)| h).collect();
    handles.sort_unstable();
    let primary = world.investor.primary_group;
    let name = chapter_name(world, dataset, number);
    let mut log_firm = None;
    for firm in handles {
        if !offered_to(world, firm) || world.chapters.active.contains_key(&firm) {
            continue;
        }
        let kinds = offer_kinds(world, dataset, firm, number);
        if Some(firm) == primary {
            log_firm = Some((firm, kinds.clone()));
        }
        if !kinds.is_empty() {
            world.chapters.offers.insert(
                firm,
                Offer {
                    chapter: number,
                    set: start,
                    kinds,
                },
            );
        }
    }
    if primary.is_some() {
        let (firm, offered) = match log_firm {
            Some((firm, kinds)) => (Some(firm), kinds),
            None => (primary, Vec::new()),
        };
        world.chapters.log.push(ChapterLog {
            number,
            name,
            start,
            firm,
            offered,
            chosen: None,
            outcome: None,
            closed: 0,
            bonus_cents: 0,
            reputation_bps: 0,
        });
        if world.chapters.log.len() > LOG_KEPT {
            world.chapters.log.remove(0);
        }
    }
}

/// Settle every mandate whose chapter has ended.
fn close_due(world: &mut World, dataset: &Dataset) {
    let t = table::mandate_table();
    let due: Vec<Handle> = world
        .chapters
        .active
        .iter()
        .filter(|(_, m)| world.tick.0 >= m.due)
        .map(|(h, _)| *h)
        .collect();
    for firm in due {
        let mandate = world.chapters.active.remove(&firm).expect("listed above");
        let operating = world.firms.get(firm).is_some_and(|f| f.operative());
        let player = player_side(world, firm);
        let (outcome, bonus, reputation) = if !operating {
            (Outcome::Cancelled, 0, 0)
        } else if succeeded(world, dataset, &mandate) {
            let bonus = if player { pay_bonus(world, firm, t) } else { 0 };
            (
                Outcome::Met,
                bonus,
                apply_reputation(world, firm, t.rewards.reputation_success_bps),
            )
        } else {
            (
                Outcome::Missed,
                0,
                apply_reputation(world, firm, t.rewards.reputation_failure_bps),
            )
        };
        if !player && outcome != Outcome::Cancelled {
            let record = world.chapters.records.entry(firm).or_insert((0, 0));
            if outcome == Outcome::Met {
                record.0 = record.0.saturating_add(1);
            } else {
                record.1 = record.1.saturating_add(1);
            }
        }
        if let Some(entry) = world
            .chapters
            .log
            .iter_mut()
            .rev()
            .find(|c| c.number == mandate.chapter && c.firm == Some(firm))
        {
            entry.outcome = Some(outcome);
            entry.closed = world.tick.0;
            entry.bonus_cents = bonus;
            entry.reputation_bps = reputation;
        }
    }
    let current = chapter_of(world.tick.0);
    for entry in world.chapters.log.iter_mut() {
        if entry.outcome.is_none() && entry.chosen.is_none() && entry.number < current {
            entry.outcome = Some(Outcome::Lapsed);
            entry.closed = world.tick.0;
        }
    }
    // Offers nobody took lapse with the chapter's window; the player's chapter log says so.
    let window = t.offer_window_weeks;
    let lapsed: Vec<Handle> = world
        .chapters
        .offers
        .iter()
        .filter(|(_, o)| world.tick.0 >= o.set + window)
        .map(|(h, _)| *h)
        .collect();
    for firm in lapsed {
        let offer = world.chapters.offers.remove(&firm).expect("listed above");
        if let Some(entry) = world.chapters.log.iter_mut().rev().find(|c| {
            c.number == offer.chapter
                && c.firm == Some(firm)
                && c.chosen.is_none()
                && c.outcome.is_none()
        }) {
            entry.outcome = Some(Outcome::Lapsed);
            entry.closed = world.tick.0;
        }
    }
}

fn apply_reputation(world: &mut World, firm: Handle, delta: i32) -> i32 {
    let t = table::mandate_table();
    let Some(entity) = world.firms.get_mut(firm) else {
        return 0;
    };
    let before = entity.reputation_bps;
    entity.reputation_bps = (before + delta).clamp(
        t.rewards.reputation_floor_bps.min(before),
        t.rewards.reputation_ceiling_bps.max(before),
    );
    entity.reputation_bps - before
}

/// Pay the management bonus from the company to the investor: the rate on a year of operating
/// profit (the last quarter, four times), at least the minimum, never more than free cash. It
/// moves money; it never mints it.
fn pay_bonus(world: &mut World, firm: Handle, t: &MandateTable) -> i64 {
    let year = weekly_ebit(world, firm, 1).unwrap_or(0) * CHAPTER_WEEKS as i64;
    let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
    let bonus = (year.max(0) * t.rewards.bonus_bps / 10_000)
        .max(t.rewards.bonus_min_cents)
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

/// Latch event mandates and test the cash line, once a week.
fn watch(world: &mut World, dataset: &Dataset) {
    let t = table::mandate_table();
    let firms: Vec<Handle> = world.chapters.active.keys().copied().collect();
    for firm in firms {
        let mandate = world.chapters.active[&firm].clone();
        let mut met = mandate.met;
        let mut broken = mandate.broken;
        match &mandate.kind {
            MandateKind::Integrate { product } => {
                met = met || products_made(world, dataset, firm, true).contains(product);
            }
            MandateKind::ListSubsidiary { baseline } => {
                met = met || listed_members(world, firm) > *baseline;
            }
            MandateKind::WinRaid => {
                met = met
                    || world
                        .raids
                        .iter()
                        .any(|r| r.attacker == firm && r.success && r.tick >= mandate.set);
            }
            MandateKind::CashCover { cover_weeks } => {
                if let Some(cost) = weekly_costs(world, firm, t.min_weeks_of_history) {
                    let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
                    if cost > 0 && cash < cost * *cover_weeks as i64 {
                        broken = true;
                    }
                }
            }
            _ => {}
        }
        if let Some(stored) = world.chapters.active.get_mut(&firm) {
            stored.met = met;
            stored.broken = broken;
        }
    }
}

/// The weekly step, after the week's books have closed.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    // A leftover 1.2 quarterly goal from an older save is retired, not paid and not penalised.
    world.objective = None;
    let now = world.tick.0;
    if now == 0 {
        return;
    }
    watch(world, dataset);
    close_due(world, dataset);
    let number = chapter_of(now);
    if world.chapters.chapter < number {
        open_chapter(world, dataset, number);
    }
    achievements::weekly(world, dataset);
}

/// Whether the player's board has a pending offer, for the pause-free reminder in the app.
pub fn player_offer(world: &World) -> Option<&Offer> {
    world
        .investor
        .primary_group
        .and_then(|f| world.chapters.offers.get(&f))
}
