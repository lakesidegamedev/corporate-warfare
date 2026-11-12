//! Careers (2.0): how a career starts, what it plays for and how it ends.
//!
//! A career is a new-game option on top of any scenario. Without one (`World.career` is `None`)
//! the game is exactly the sandbox or five-year game it always was. With one:
//!
//! - **Start.** Founder (the scenario's fortune, found a company) or At the helm: the player buys
//!   a controlling block of an established company from the league's middle at the opening mark.
//!   The block is bought from the firm's outside holders through the exchange's own settlement,
//!   so the sellers are paid and nothing is created. If the standard fortune cannot pay for the
//!   block, the fortune is raised to the content cover of the price (scaled by difficulty) and
//!   the career records the factor. The sitting chief executive leaves; the board stays. From
//!   week 0 the firm is the player's: every rule that asks whether a firm is the player's reads
//!   the controlling holder, so the AI, executives, letters and chapters follow.
//! - **Ambition.** One of four, judged by the engine with bronze, silver and gold thresholds
//!   (`content/careers.json`). Richest and Biggest are judged at the end; Dominant is won the
//!   moment the gold share is held for the content number of quarters; Survivor at the end.
//! - **Ending.** At the end of the length, at Dominant gold, or lost at any time: no company and
//!   too little fortune to found one, removed by the board after its warning, or a rival took
//!   control of the player's company. After an ending the world plays on as a sandbox.
//! - **Summary.** A year by year line, the events that mattered and one plain sentence, all from
//!   engine facts kept in the state.
//!
//! The state lives outside the postcard payload (`World.career`, save trailer `CARE`) and is
//! folded into the canonical hash while set, so a world without a career hashes as before.
//! Nothing here draws a random number.

use std::collections::{BTreeMap, BTreeSet};

use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_data::careers::{CareerTable, career_table};
use serde::{Deserialize, Serialize};

use crate::economy::ownership::ultimate_controller;
use crate::state::{FacilityStatus, FirmStatus, HolderId, NewsItem, World};

pub const QUARTER_WEEKS: u32 = 13;
pub const YEAR_WEEKS: u32 = 52;
/// The four-week period the board reads results in (the rival rule's period).
const PERIOD_WEEKS: u32 = crate::state::PERIOD_WEEKS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Start {
    #[default]
    Founder,
    Helm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ambition {
    Richest,
    Biggest,
    Dominant,
    Survivor,
}

impl Ambition {
    pub const ALL: [Ambition; 4] = [
        Ambition::Richest,
        Ambition::Biggest,
        Ambition::Dominant,
        Ambition::Survivor,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Ambition::Richest => "richest",
            Ambition::Biggest => "biggest",
            Ambition::Dominant => "dominant",
            Ambition::Survivor => "survivor",
        }
    }

    pub fn from_key(key: &str) -> Option<Ambition> {
        Ambition::ALL.into_iter().find(|a| a.key() == key)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Medal {
    #[default]
    None,
    Bronze,
    Silver,
    Gold,
}

impl Medal {
    pub const fn key(self) -> &'static str {
        match self {
            Medal::None => "none",
            Medal::Bronze => "bronze",
            Medal::Silver => "silver",
            Medal::Gold => "gold",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndKind {
    /// The career ran its length.
    Completed,
    /// The ambition's gold was reached before the end (Dominant).
    Won,
    /// No company and too little fortune to found or buy one.
    NoCompany,
    /// The board removed the player after its warning.
    Removed,
    /// A rival took control of the player's company.
    TakenOver,
}

impl EndKind {
    pub const fn key(self) -> &'static str {
        match self {
            EndKind::Completed => "completed",
            EndKind::Won => "won",
            EndKind::NoCompany => "no_company",
            EndKind::Removed => "removed",
            EndKind::TakenOver => "taken_over",
        }
    }

    pub const fn lost(self) -> bool {
        matches!(
            self,
            EndKind::NoCompany | EndKind::Removed | EndKind::TakenOver
        )
    }
}

/// The new-game option. `ambition: None` is a sandbox career with no goal and no ending.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CareerSetup {
    pub start: Start,
    /// The seed firm picked from `helm_offers` (its handle). Required for `Start::Helm`.
    pub helm_firm: Option<u64>,
    pub ambition: Option<Ambition>,
    /// Career length in years; 0 takes the content default.
    pub years: u32,
}

/// The numbers a career is judged by, copied from content when it starts so a later content
/// change does not move a running career's goalposts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    /// Biggest for this career's start: a top share of the league that earns bronze (0: none)
    /// and the places for bronze (0: only the top share), silver and gold.
    pub rank_bronze_top_bps: i64,
    pub rank_bronze: u32,
    pub rank_silver: u32,
    pub rank_gold: u32,
    /// Richest for this career's length: net worth over the starting fortune, basis points.
    pub richest_bronze_bps: i64,
    pub richest_silver_bps: i64,
    pub richest_gold_bps: i64,
    pub dominant_quarters: u32,
    pub dominant_min_producers: u32,
    pub dominant_bronze_bps: i64,
    pub dominant_silver_bps: i64,
    pub dominant_gold_bps: i64,
    pub survivor_silver_top_bps: i64,
    pub survivor_gold_top_bps: i64,
    pub fortune_floor_cents: i64,
    pub found_grace_weeks: u32,
    pub board_grace_weeks: u32,
    pub board_loss_periods: u32,
    pub board_warning_weeks: u32,
    pub events_kept: u32,
}

impl Rules {
    #[cfg(test)]
    fn from_table(t: &CareerTable, start: Start, years: u32) -> Rules {
        Rules::from_table_in(t, start, years, "")
    }

    /// The rules for a career in `scenario` (its own Richest multiples where content sets them).
    fn from_table_in(t: &CareerTable, start: Start, years: u32, scenario: &str) -> Rules {
        let places = match start {
            Start::Founder => &t.biggest.founder,
            Start::Helm => &t.biggest.helm,
        };
        let multiples = t.richest.for_scenario_years(scenario, years);
        Rules {
            rank_bronze_top_bps: places.bronze_top_bps,
            rank_bronze: places.bronze,
            rank_silver: places.silver,
            rank_gold: places.gold,
            richest_bronze_bps: multiples.bronze_bps,
            richest_silver_bps: multiples.silver_bps,
            richest_gold_bps: multiples.gold_bps,
            dominant_quarters: t.dominant.quarters,
            dominant_min_producers: t.dominant.min_producers,
            dominant_bronze_bps: t.dominant.bronze_bps,
            dominant_silver_bps: t.dominant.silver_bps,
            dominant_gold_bps: t.dominant.gold_bps,
            survivor_silver_top_bps: t.survivor.silver_top_bps,
            survivor_gold_top_bps: t.survivor.gold_top_bps,
            fortune_floor_cents: t.fortune_floor_cents,
            found_grace_weeks: t.found_grace_weeks,
            board_grace_weeks: t.board.grace_weeks,
            board_loss_periods: t.board.loss_periods,
            board_warning_weeks: t.board.warning_weeks,
            events_kept: t.events_kept as u32,
        }
    }
}

/// What the helm start bought.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelmTaken {
    pub firm: Handle,
    pub name: String,
    pub rank: u32,
    pub shares: i64,
    pub price_cents: i64,
    pub paid_cents: i64,
    /// The scenario's standard fortune and the fortune the career started with.
    pub standard_fortune_cents: i64,
    pub fortune_cents: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearLine {
    pub year: u32,
    pub week: u32,
    /// The primary company's league rank by value; `None` when the player runs none.
    pub rank: Option<u32>,
    pub league: u32,
    pub company_value_cents: i64,
    pub net_worth_cents: i64,
    pub wealth_rank: u32,
    pub medal: Medal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CareerEvent {
    pub week: u32,
    pub key: String,
    pub text: String,
    /// Weight for picking the events that mattered (larger matters more).
    pub weight: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ending {
    pub week: u32,
    pub kind: EndKind,
    pub medal: Medal,
    pub sentence: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracking {
    pub plants_seen: u32,
    pub raids_seen: u32,
    pub chapters_seen: u32,
    pub group_seen: BTreeSet<Handle>,
    pub past_due: BTreeSet<Handle>,
    /// Lots made this quarter by industry: (the player's group, everyone).
    pub quarter_lots: BTreeMap<u8, (i64, i64)>,
    /// The top-level owners (the player's group as one) that made each industry's output this
    /// quarter.
    pub quarter_producers: BTreeMap<u8, BTreeSet<u64>>,
    /// Quarters in a row each industry's share has been at or above each medal's line.
    pub streaks: BTreeMap<u8, [u32; 3]>,
    /// Best Dominant medal reached so far, its industry and share.
    pub dominant_best: Medal,
    pub dominant_industry: Option<u8>,
    pub dominant_share_bps: i64,
    /// The board: the week its warning was given, and the last period read.
    pub warned_week: Option<u32>,
    pub board_confidence: Option<bool>,
    pub had_company: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CareerState {
    pub start: Start,
    pub ambition: Option<Ambition>,
    pub years: u32,
    pub start_week: u32,
    pub end_week: u32,
    pub rules: Rules,
    pub helm: Option<HelmTaken>,
    pub years_log: Vec<YearLine>,
    pub events: Vec<CareerEvent>,
    pub tracking: Tracking,
    pub ending: Option<Ending>,
    /// The fortune Richest measures growth from: the net worth when the career started (a
    /// helm start: just after buying control, so the scaled fortune is not counted as a gain).
    pub start_worth_cents: i64,
}

// ----- leagues -----

/// A company's value as the league reads it: the valuation's equity once computed, the book
/// equity before the first valuation.
pub fn company_value(firm: &crate::state::Firm) -> i64 {
    if firm.valuation.computed_week.0 == 0 && firm.valuation.equity_cents == 0 {
        firm.book.total_equity()
    } else {
        firm.valuation.equity_cents
    }
}

/// The company league: every operating company that no other company controls, by value, largest
/// first (ties by handle). The player's group counts once, as its primary company.
pub fn company_league(world: &World) -> Vec<(Handle, i64)> {
    let primary = world.investor.primary_group;
    let mut rows = Vec::new();
    for (handle, firm) in world.firms.iter() {
        if !firm.status.operative() {
            continue;
        }
        let controller = ultimate_controller(world, handle);
        if matches!(controller, Some(HolderId::Firm(_))) {
            continue;
        }
        if controller == Some(HolderId::Investor) && Some(handle) != primary {
            continue;
        }
        rows.push((handle, company_value(firm)));
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}

pub fn rank_in(rows: &[(Handle, i64)], handle: Handle) -> Option<u32> {
    rows.iter()
        .position(|(h, _)| *h == handle)
        .map(|i| i as u32 + 1)
}

/// The wealth league: the player and every outside investor in the corridor, each valued as
/// cash (capital) plus every share held at the fair price, minus personal loans for the player.
/// Rival chief executives keep no personal purse in the engine, so they are not in it.
/// Returns `(player net worth, player rank, league size)`.
pub fn wealth_league(world: &World) -> (i64, u32, u32) {
    let mut outside: BTreeMap<u32, i64> = world
        .outside
        .investors
        .iter()
        .enumerate()
        .map(|(i, h)| (i as u32, h.capital_cents.max(0)))
        .collect();
    for (_, firm) in world.firms.iter() {
        for (holder, lot) in &firm.shares.holders {
            if let HolderId::Outside(i) = holder
                && let Some(worth) = outside.get_mut(i)
            {
                *worth += lot.qty.saturating_mul(firm.shares.fair_cents);
            }
        }
    }
    let ours = crate::economy::score::personal_net_worth(world);
    let above = outside.values().filter(|w| **w > ours).count() as u32;
    (ours, above + 1, outside.len() as u32 + 1)
}

fn group(world: &World) -> BTreeSet<Handle> {
    world
        .firms
        .iter()
        .filter(|(h, f)| {
            f.status.operative() && ultimate_controller(world, *h) == Some(HolderId::Investor)
        })
        .map(|(h, _)| h)
        .collect()
}

// ----- the helm start -----

/// One seed firm the helm start offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelmOffer {
    pub firm: Handle,
    pub name: String,
    pub industry: u8,
    pub region: u8,
    pub rank: u32,
    pub value_cents: i64,
    pub revenue_year_cents: i64,
    pub plants: u32,
    pub workers: i64,
    /// Shares the player must buy for control, at the opening mark, and their price.
    pub shares_needed: i64,
    pub mark_cents: i64,
    pub price_cents: i64,
}

/// Shares a buyer holding `held` needs to add for control of `book`.
fn shares_for_control(book: &crate::state::ShareBook, held: i64) -> i64 {
    let eligible = book.eligible_votes();
    let need = (eligible as i128 * crate::state::CONTROL_NUMERATOR as i128
        / crate::state::CONTROL_DENOMINATOR as i128) as i64
        + 1;
    (need - held).max(0)
}

fn outside_held(book: &crate::state::ShareBook) -> i64 {
    book.holders
        .iter()
        .filter(|(h, _)| matches!(h, HolderId::Outside(_)))
        .map(|(_, l)| l.qty)
        .sum()
}

fn offer_for(world: &World, handle: Handle, rank: u32) -> Option<HelmOffer> {
    let firm = world.firms.get(handle)?;
    if !matches!(firm.status, FirmStatus::Operating) {
        return None;
    }
    let controller = firm.shares.controller();
    if matches!(
        controller,
        Some(HolderId::Firm(_)) | Some(HolderId::State) | Some(HolderId::Investor)
    ) {
        return None;
    }
    let needed = shares_for_control(&firm.shares, firm.shares.qty(HolderId::Investor));
    if needed == 0 || outside_held(&firm.shares) < needed {
        return None;
    }
    let mark = if firm.shares.mark_cents > 0 {
        firm.shares.mark_cents
    } else {
        firm.shares.fair_cents
    };
    if mark <= 0 {
        return None;
    }
    let plants: Vec<&crate::state::Facility> = firm
        .facilities
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .collect();
    let revenue_week = firm
        .book
        .recent_weeks()
        .last()
        .map(|w| w.revenue)
        .unwrap_or(0);
    Some(HelmOffer {
        firm: handle,
        name: firm.name.clone(),
        industry: firm.industry,
        region: firm.region,
        rank,
        value_cents: company_value(firm),
        revenue_year_cents: revenue_week.saturating_mul(YEAR_WEEKS as i64),
        plants: plants.len() as u32,
        workers: plants.iter().map(|p| p.workers.iter().sum::<i64>()).sum(),
        shares_needed: needed,
        mark_cents: mark,
        price_cents: needed.saturating_mul(mark),
    })
}

/// The seed firms the helm start offers: from the content rank band of the company league,
/// different sectors first, then by rank. Firms another company or the state controls, or whose
/// outside holders do not hold enough shares to sell control, are passed over.
pub fn helm_offers(world: &World) -> Vec<HelmOffer> {
    helm_offers_in(world, None)
}

/// As `helm_offers`, but from a band of ranks the caller names (`cw-cli arc --start helm:RANK`).
pub fn helm_offers_in(world: &World, band: Option<(u32, u32)>) -> Vec<HelmOffer> {
    let table = career_table();
    let (from, to) = band.unwrap_or((table.helm.rank_from, table.helm.rank_to));
    let league = company_league(world);
    let eligible: Vec<HelmOffer> = league
        .iter()
        .enumerate()
        .map(|(i, (h, _))| (i as u32 + 1, *h))
        .filter(|(rank, _)| (from..=to).contains(rank))
        .filter_map(|(rank, h)| offer_for(world, h, rank))
        .collect();
    let want = table.helm.offers as usize;
    let mut picked: Vec<HelmOffer> = Vec::new();
    let mut sectors: BTreeSet<u8> = BTreeSet::new();
    for offer in &eligible {
        if picked.len() < want && sectors.insert(offer.industry) {
            picked.push(offer.clone());
        }
    }
    for offer in &eligible {
        if picked.len() >= want {
            break;
        }
        if !picked.iter().any(|p| p.firm == offer.firm) {
            picked.push(offer.clone());
        }
    }
    picked.sort_by_key(|o| o.rank);
    picked
}

/// The fortune the helm start gives for a block priced `price`: the standard fortune when it
/// covers the price, else the content cover of the price scaled by difficulty, never less than
/// the price itself.
pub fn helm_fortune(standard: i64, price: i64, difficulty: cw_data::Difficulty) -> i64 {
    if standard >= price {
        return standard;
    }
    let t = &career_table().helm;
    let cover = (price as i128 * t.fortune_cover_bps as i128 / 10_000) as i64;
    let scaled = (cover as i128 * t.fortune_difficulty_bps.get(difficulty) as i128 / 10_000) as i64;
    scaled.max(price)
}

fn push_event(state: &mut CareerState, week: u32, key: &str, text: String, weight: i64) {
    state.events.push(CareerEvent {
        week,
        key: key.to_owned(),
        text,
        weight,
    });
}

/// Start a career on a freshly built world. Returns the reason in plain words when the setup
/// cannot be played (a helm pick that is not on offer).
pub fn begin(world: &mut World, ds: &Dataset, setup: &CareerSetup) -> Result<(), String> {
    let table = career_table();
    // The five-year score mode is the five-year length of a career: a five-year game started
    // with a career and no length plays five years, and its score sheet is unchanged.
    let years = if setup.years == 0 && world.mode == crate::state::ScenarioMode::FiveYear {
        5
    } else if setup.years == 0 {
        table.default_years
    } else {
        setup.years
    };
    if !table.lengths_years.contains(&years) {
        return Err(format!(
            "A career lasts one of {:?} years",
            table.lengths_years
        ));
    }
    let start_week = world.tick.0;
    let mut state = CareerState {
        start: setup.start,
        ambition: setup.ambition,
        years,
        start_week,
        end_week: start_week + years * YEAR_WEEKS,
        rules: Rules::from_table_in(table, setup.start, years, &ds.scenario_id),
        start_worth_cents: crate::economy::score::personal_net_worth(world),
        helm: None,
        years_log: Vec::new(),
        events: Vec::new(),
        tracking: Tracking::default(),
        ending: None,
    };
    if setup.start == Start::Helm {
        let pick = setup
            .helm_firm
            .ok_or_else(|| "Pick one of the companies on offer".to_owned())?;
        let league = company_league(world);
        let rank = rank_in(&league, Handle(pick))
            .ok_or_else(|| "That company is not in the league".to_owned())?;
        let offer = offer_for(world, Handle(pick), rank)
            .ok_or_else(|| "Control of that company is not for sale".to_owned())?;
        let standard = world.investor.cash_cents;
        let fortune = helm_fortune(standard, offer.price_cents, world.difficulty);
        world.investor.cash_cents = fortune;
        // Buy the block from the outside holders, largest first, at the opening mark.
        let mut left = offer.shares_needed;
        while left > 0 {
            let lot = world.firms.get(offer.firm).and_then(|f| {
                f.shares
                    .holders
                    .iter()
                    .filter(|(h, l)| matches!(h, HolderId::Outside(_)) && l.qty > 0)
                    .max_by(|a, b| a.1.qty.cmp(&b.1.qty).then(b.0.cmp(a.0)))
                    .map(|(h, l)| (*h, l.qty))
            });
            let Some((holder, held)) = lot else { break };
            let chunk = left.min(held);
            crate::economy::equity::transfer_paid(
                world,
                offer.firm,
                holder,
                HolderId::Investor,
                chunk,
                chunk.saturating_mul(offer.mark_cents),
            );
            left -= chunk;
        }
        if world
            .firms
            .get(offer.firm)
            .and_then(|f| f.shares.controller())
            != Some(HolderId::Investor)
        {
            return Err("Control of that company could not be bought".to_owned());
        }
        world.investor.primary_group = Some(offer.firm);
        if !world.investor.managed_firms.contains(&offer.firm) {
            world.investor.managed_firms.push(offer.firm);
        }
        // The sitting chief executive leaves; the player takes the office. The board stays.
        if let Some(f) = world.firms.get_mut(offer.firm)
            && let Some(old) = f.ceo.take()
        {
            world.cast.appointed.remove(&offer.firm);
            world
                .cast
                .benched
                .insert(old, world.tick.0 + crate::economy::cast::BENCH_WEEKS);
            if (old as usize) < ds.cast.len() {
                let text = format!(
                    "{} steps down as chief executive of {} as a new controlling owner takes the helm.",
                    ds.cast[old as usize].name, offer.name
                );
                world.politics.news.push(crate::state::PoliticalNews {
                    week: world.tick.0,
                    text,
                    person: None,
                });
            }
        }
        world.investor.starting_net_worth_cents = crate::economy::score::personal_net_worth(world);
        state.start_worth_cents = world.investor.starting_net_worth_cents;
        let paid = offer.price_cents;
        push_event(
            &mut state,
            start_week,
            "took_helm",
            format!(
                "Took the helm of {} (rank {} of {}), buying {} shares for {}.",
                offer.name,
                rank,
                league.len(),
                offer.shares_needed,
                money(paid)
            ),
            i64::MAX / 2,
        );
        state.helm = Some(HelmTaken {
            firm: offer.firm,
            name: offer.name.clone(),
            rank,
            shares: offer.shares_needed,
            price_cents: offer.mark_cents,
            paid_cents: paid,
            standard_fortune_cents: standard,
            fortune_cents: fortune,
        });
        state.tracking.had_company = true;
        state.tracking.group_seen = group(world);
    }
    world.career = Some(state);
    Ok(())
}

fn money(cents: i64) -> String {
    let dollars = cents / 100;
    if dollars.abs() >= 1_000_000 {
        format!(
            "${}.{}M",
            dollars / 1_000_000,
            (dollars.abs() / 100_000) % 10
        )
    } else if dollars.abs() >= 1_000 {
        format!("${}k", dollars / 1_000)
    } else {
        format!("${dollars}")
    }
}

// ----- judging -----

/// Net worth over the starting fortune, basis points (10000 = no growth; 0 when the start was
/// not positive).
pub fn growth_bps(worth: i64, start: i64) -> i64 {
    if start <= 0 {
        return 0;
    }
    (worth.max(0) as i128 * 10_000 / start as i128).min(i64::MAX as i128) as i64
}

/// The medal the ambition stands at right now (for Dominant: the best held so far).
pub fn standing(world: &World, state: &CareerState) -> Medal {
    let r = &state.rules;
    let Some(ambition) = state.ambition else {
        return Medal::None;
    };
    let league = company_league(world);
    let primary = live_primary(world);
    let n = league.len() as i64;
    // Rank r is in the top share s when r <= ceil(n * s).
    let within = |rank: u32, bps: i64| (rank as i64) * 10_000 <= (n * bps).max(10_000) + 9_999;
    match ambition {
        Ambition::Richest => {
            let (worth, _, _) = wealth_league(world);
            let growth = growth_bps(worth, state.start_worth_cents);
            if growth >= r.richest_gold_bps {
                Medal::Gold
            } else if growth >= r.richest_silver_bps {
                Medal::Silver
            } else if growth >= r.richest_bronze_bps {
                Medal::Bronze
            } else {
                Medal::None
            }
        }
        Ambition::Biggest => match primary.and_then(|p| rank_in(&league, p)) {
            Some(x) if x <= r.rank_gold => Medal::Gold,
            Some(x) if x <= r.rank_silver => Medal::Silver,
            Some(x)
                if x <= r.rank_bronze
                    || (r.rank_bronze_top_bps > 0 && within(x, r.rank_bronze_top_bps)) =>
            {
                Medal::Bronze
            }
            _ => Medal::None,
        },
        Ambition::Dominant => state.tracking.dominant_best,
        Ambition::Survivor => {
            let Some(rank) = primary.and_then(|p| rank_in(&league, p)) else {
                return Medal::None;
            };
            if within(rank, r.survivor_gold_top_bps) {
                Medal::Gold
            } else if within(rank, r.survivor_silver_top_bps) {
                Medal::Silver
            } else {
                Medal::Bronze
            }
        }
    }
}

/// Where a career stands, in plain words, for the screens and the quarterly letter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub ambition: Option<Ambition>,
    pub medal: Medal,
    /// The primary company's league place and the league's size (0 when it runs none).
    pub rank: u32,
    pub league: u32,
    /// Richest: net worth over the starting fortune, basis points (10000 = no growth).
    pub growth_bps: i64,
    /// What the standing rests on, one sentence.
    pub detail: String,
    /// What the next medal needs, one sentence; empty at gold or without an ambition.
    pub next: String,
}

fn times(bps: i64) -> String {
    format!("{}.{}", bps / 10_000, (bps % 10_000) / 1_000)
}

fn place_words(n: u32) -> String {
    match n {
        1 => "first place".to_owned(),
        _ => format!("the top {n}"),
    }
}

/// The career's standing now: the medal, the facts it rests on and what the next medal needs.
pub fn progress(world: &World, state: &CareerState) -> Progress {
    let r = &state.rules;
    let medal = standing(world, state);
    let league = company_league(world);
    let rank = live_primary(world).and_then(|p| rank_in(&league, p));
    let n = league.len() as u32;
    let (worth, _, _) = wealth_league(world);
    let growth = growth_bps(worth, state.start_worth_cents);
    let mut out = Progress {
        ambition: state.ambition,
        medal,
        rank: rank.unwrap_or(0),
        league: n,
        growth_bps: growth,
        detail: String::new(),
        next: String::new(),
    };
    let Some(ambition) = state.ambition else {
        return out;
    };
    let next = match medal {
        Medal::None => Some(Medal::Bronze),
        Medal::Bronze => Some(Medal::Silver),
        Medal::Silver => Some(Medal::Gold),
        Medal::Gold => None,
    };
    match ambition {
        Ambition::Richest => {
            out.detail = format!(
                "Your net worth of {} is {} times the {} you started with.",
                money(worth),
                times(growth),
                money(state.start_worth_cents)
            );
            if let Some(m) = next {
                let need = match m {
                    Medal::Bronze => r.richest_bronze_bps,
                    Medal::Silver => r.richest_silver_bps,
                    _ => r.richest_gold_bps,
                };
                let target =
                    (state.start_worth_cents.max(0) as i128 * need as i128 / 10_000) as i64;
                out.next = format!(
                    "{} needs {} times, a net worth of {} at the end.",
                    capital(m.key()),
                    times(need),
                    money(target)
                );
            }
        }
        Ambition::Biggest | Ambition::Survivor => {
            out.detail = match rank {
                Some(x) => format!("Your company ranks {x} of {n} by value."),
                None => "You run no company in the league.".to_owned(),
            };
            if let Some(m) = next {
                let places = if ambition == Ambition::Biggest {
                    match m {
                        Medal::Bronze if r.rank_bronze == 0 => {
                            let top =
                                ((n as i64 * r.rank_bronze_top_bps).max(10_000) + 9_999) / 10_000;
                            top as u32
                        }
                        Medal::Bronze => r.rank_bronze,
                        Medal::Silver => r.rank_silver,
                        _ => r.rank_gold,
                    }
                } else {
                    let bps = match m {
                        Medal::Bronze => 10_000,
                        Medal::Silver => r.survivor_silver_top_bps,
                        _ => r.survivor_gold_top_bps,
                    };
                    (((n as i64 * bps).max(10_000) + 9_999) / 10_000) as u32
                };
                out.next = if ambition == Ambition::Survivor && m == Medal::Bronze {
                    "Bronze needs a solvent company you control at the end.".to_owned()
                } else {
                    format!(
                        "{} needs {} by value at the end.",
                        capital(m.key()),
                        place_words(places)
                    )
                };
            }
        }
        Ambition::Dominant => {
            out.detail = if state.tracking.dominant_best == Medal::None {
                "Your group has not yet held a leading share of an industry for a year.".to_owned()
            } else {
                format!(
                    "Your group's best hold so far is {}.{} percent of {}.",
                    state.tracking.dominant_share_bps / 100,
                    (state.tracking.dominant_share_bps % 100) / 10,
                    industry_name(state.tracking.dominant_industry.unwrap_or(0))
                )
            };
            if let Some(m) = next {
                let bps = match m {
                    Medal::Bronze => r.dominant_bronze_bps,
                    Medal::Silver => r.dominant_silver_bps,
                    _ => r.dominant_gold_bps,
                };
                out.next = format!(
                    "{} needs {} percent of one industry's output for {} quarters in a row.",
                    capital(m.key()),
                    bps / 100,
                    r.dominant_quarters
                );
            }
        }
    }
    out
}

fn capital(word: &str) -> String {
    let mut c = word.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The player's primary company if it is operating, solvent and the player's.
fn live_primary(world: &World) -> Option<Handle> {
    let p = world.investor.primary_group?;
    let firm = world.firms.get(p)?;
    if !firm.status.operative() || ultimate_controller(world, p) != Some(HolderId::Investor) {
        return None;
    }
    Some(p)
}

fn alert(world: &mut World, key: &str, subject: String) {
    let week = world.tick;
    world.news.push(NewsItem {
        week,
        key: key.to_owned(),
        subject,
        amount_cents: 0,
        quantity: cw_core::quantity::MilliLots(0),
    });
}

/// Weekly: track the facts a career summary needs, read the board, judge the ambition each
/// quarter and decide whether the career has ended. Does nothing without a career.
pub fn weekly(world: &mut World, ds: &Dataset) {
    let Some(mut state) = world.career.take() else {
        return;
    };
    if state.ending.is_none() {
        step(world, ds, &mut state);
    }
    world.career = Some(state);
}

fn step(world: &mut World, ds: &Dataset, state: &mut CareerState) {
    let week = world.tick.0;
    let elapsed = week.saturating_sub(state.start_week);
    let groups = group(world);

    // Firsts and companies bought.
    let plants = groups
        .iter()
        .filter_map(|h| world.firms.get(*h))
        .flat_map(|f| f.facilities.iter())
        .filter_map(|h| world.facilities.get(*h))
        .filter(|p| p.status == FacilityStatus::Operating)
        .count() as u32;
    if state.tracking.plants_seen == 0 && plants > 0 && state.start == Start::Founder {
        push_event(
            state,
            week,
            "first_plant",
            "Your first plant started work.".into(),
            1_000_000_000_000,
        );
    }
    state.tracking.plants_seen = plants;
    for h in &groups {
        if !state.tracking.group_seen.contains(h) && state.tracking.had_company {
            let (name, value) = world
                .firms
                .get(*h)
                .map(|f| (f.name.clone(), company_value(f)))
                .unwrap_or_default();
            push_event(
                state,
                week,
                "company_bought",
                format!("{name} joined your group, worth {}.", money(value)),
                value,
            );
        }
    }
    for h in state.tracking.group_seen.clone() {
        if groups.contains(&h) {
            continue;
        }
        let Some(f) = world.firms.get(h) else {
            continue;
        };
        let (key, text) = match f.status {
            FirmStatus::Liquidating | FirmStatus::Dissolved => (
                "company_failed",
                format!("{} failed and was wound up.", f.name),
            ),
            FirmStatus::Merged { .. } => ("company_merged", format!("{} was merged away.", f.name)),
            _ => ("company_lost", format!("{} left your control.", f.name)),
        };
        push_event(state, week, key, text, 40_000_000_000);
    }
    if !groups.is_empty() {
        state.tracking.had_company = true;
    }
    state.tracking.group_seen = groups.clone();

    // Raids won and lost.
    let total = world.raids.log.len() as u32;
    let start = state.tracking.raids_seen.min(total) as usize;
    let fresh: Vec<crate::state::RaidRecord> = world.raids.log[start..].to_vec();
    state.tracking.raids_seen = total;
    for raid in fresh {
        let ours_attack = groups.contains(&raid.attacker);
        let ours_defend = groups.contains(&raid.defender);
        if !(ours_attack || ours_defend) {
            continue;
        }
        let other = if ours_attack {
            raid.defender
        } else {
            raid.attacker
        };
        let name = world
            .firms
            .get(other)
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let (key, text) = match (ours_attack, raid.success) {
            (true, true) => ("raid_won", format!("Won a raid on {name}'s market.")),
            (true, false) => ("raid_lost", format!("A raid on {name}'s market failed.")),
            (false, true) => ("raid_lost", format!("Lost a market to {name}'s raid.")),
            (false, false) => ("raid_won", format!("Fought off a raid by {name}.")),
        };
        push_event(
            state,
            raid.tick,
            key,
            text,
            raid.budget_cents.max(raid.price_cents),
        );
    }

    // Mandates met and failed.
    let logs = world.chapters.log.len() as u32;
    for chapter in world
        .chapters
        .log
        .iter()
        .skip(state.tracking.chapters_seen.min(logs) as usize)
    {
        let Some(outcome) = chapter.outcome else {
            continue;
        };
        let (key, text) = match outcome {
            crate::economy::chapters::Outcome::Met => (
                "mandate_met",
                format!("Met the board's mandate in chapter {}.", chapter.number),
            ),
            crate::economy::chapters::Outcome::Missed => (
                "mandate_missed",
                format!("Missed the board's mandate in chapter {}.", chapter.number),
            ),
            _ => continue,
        };
        state.events.push(CareerEvent {
            week: chapter.closed,
            key: key.into(),
            text,
            weight: 5_000_000_000 + chapter.bonus_cents,
        });
    }
    // Only closed chapters are counted as seen.
    state.tracking.chapters_seen = world
        .chapters
        .log
        .iter()
        .take_while(|c| c.outcome.is_some())
        .count() as u32;

    // Crises survived: a company of the group past due and back.
    for h in &groups {
        let Some(f) = world.firms.get(*h) else {
            continue;
        };
        let due = matches!(f.status, FirmStatus::PastDue { .. });
        if due {
            state.tracking.past_due.insert(*h);
        } else if state.tracking.past_due.remove(h) {
            let name = f.name.clone();
            push_event(
                state,
                week,
                "crisis_survived",
                format!("{name} paid its way out of default."),
                20_000_000_000,
            );
        }
    }

    // Dominant: lots made by industry this quarter.
    for (_, plant) in world.facilities.iter() {
        if plant.status != FacilityStatus::Operating {
            continue;
        }
        let industry = crate::economy::sectors::recipe_industry(ds, plant.recipe);
        let lots = plant.last_output.0;
        let entry = state.tracking.quarter_lots.entry(industry).or_default();
        entry.1 += lots;
        let ours = groups.contains(&plant.firm);
        if ours {
            entry.0 += lots;
        }
        if lots > 0 {
            let owner = if ours {
                u64::MAX
            } else {
                match ultimate_controller(world, plant.firm) {
                    Some(HolderId::Firm(h)) => h.0,
                    _ => plant.firm.0,
                }
            };
            state
                .tracking
                .quarter_producers
                .entry(industry)
                .or_default()
                .insert(owner);
        }
    }

    // The board reads the company the player runs: the primary company while no one else
    // controls it, so a player who sold below half still answers to it.
    let led = world.investor.primary_group.filter(|p| {
        world.firms.get(*p).is_some_and(|f| f.status.operative())
            && matches!(
                ultimate_controller(world, *p),
                Some(HolderId::Investor) | None
            )
    });
    board(world, state, led, elapsed);
    if state.ending.is_some() {
        return;
    }

    // Quarterly: dominance streaks, progress.
    if elapsed > 0 && elapsed.is_multiple_of(QUARTER_WEEKS) {
        quarter(world, state);
        if state.ending.is_some() {
            return;
        }
    }
    // Yearly line.
    if elapsed > 0 && elapsed.is_multiple_of(YEAR_WEEKS) {
        let line = year_line(world, state, elapsed / YEAR_WEEKS);
        state.years_log.push(line);
    }

    // Lost: a rival took control of the player's company.
    if let Some(p) = world.investor.primary_group
        && world.firms.get(p).is_some_and(|f| f.status.operative())
    {
        match ultimate_controller(world, p) {
            Some(HolderId::Investor) => {}
            Some(holder) => {
                let rival = match holder {
                    HolderId::Firm(h) => world
                        .firms
                        .get(h)
                        .map(|f| f.name.clone())
                        .unwrap_or_else(|| "a rival".into()),
                    _ => "outside investors".into(),
                };
                let name = world
                    .firms
                    .get(p)
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                end(
                    world,
                    state,
                    EndKind::TakenOver,
                    format!("{rival} took control of {name}, and with it your career."),
                );
                return;
            }
            None => {}
        }
    }

    // Lost: no company and too little to found or buy one.
    let grace = state.start_week + state.rules.found_grace_weeks;
    if groups.is_empty() && week >= grace {
        let worth = crate::economy::score::personal_net_worth(world);
        if worth < state.rules.fortune_floor_cents {
            let sentence = if state.tracking.had_company {
                format!(
                    "Your last company is gone and {} is not enough to found another.",
                    money(worth)
                )
            } else {
                format!(
                    "You never built a company and {} is not enough to found one.",
                    money(worth)
                )
            };
            end(world, state, EndKind::NoCompany, sentence);
            return;
        }
    }

    // The end of the length.
    if week >= state.end_week {
        let medal = standing(world, state);
        let sentence = closing_sentence(world, state, medal);
        end(world, state, EndKind::Completed, sentence);
    }
}

/// The votes the player commands in `firm`: shares held directly and through every company
/// the player controls, against the company's eligible votes.
pub fn player_votes(world: &World, firm: Handle) -> (i64, i64) {
    let Some(f) = world.firms.get(firm) else {
        return (0, 0);
    };
    let ours: i64 = f
        .shares
        .holders
        .iter()
        .filter(|(h, _)| match h {
            HolderId::Investor => true,
            HolderId::Firm(owner) => ultimate_controller(world, *owner) == Some(HolderId::Investor),
            _ => false,
        })
        .map(|(_, l)| l.qty)
        .sum();
    (ours, f.shares.eligible_votes())
}

/// Whether the player holds half or more of the company's eligible votes. A board the player
/// controls cannot remove the player; it only warns. The same holds for rivals: a rival's
/// board replaces a hired chief executive, and rival chiefs hold no shares (`economy::cast`).
pub fn holds_majority(world: &World, firm: Handle) -> bool {
    let (ours, eligible) = player_votes(world, firm);
    eligible > 0 && ours.saturating_mul(2) >= eligible
}

/// The warning to an owner the board cannot remove, with the consequences the engine applies.
fn majority_warning(name: &str, weeks: u32) -> String {
    format!(
        "The board of {name} has lost confidence after {weeks} weeks of losses, and the outside shareholders with it. You hold the majority of the votes, so the board cannot remove you; the market can. Losses weaken the credit grade and the terms lenders offer, a long slump far under the share price's peak invites an activist shareholder, unpaid debt ends in default, and if you sell below half the board can remove you and the outside float can be bought from under you."
    )
}

fn board(world: &mut World, state: &mut CareerState, primary: Option<Handle>, elapsed: u32) {
    let Some(p) = primary else {
        return;
    };
    let week = world.tick.0;
    if elapsed < state.rules.board_grace_weeks || !week.is_multiple_of(PERIOD_WEEKS) {
        return;
    }
    let periods = state.rules.board_loss_periods as usize;
    let history = world.firm_history.get(&p);
    let since = state.start_week;
    let recent: Vec<i64> = history
        .map(|h| {
            h.iter()
                .rev()
                .take_while(|c| c.first_week >= since)
                .take(periods)
                .map(|c| c.net_income)
                .collect()
        })
        .unwrap_or_default();
    let last_profitable = recent.first().is_some_and(|n| *n >= 0);
    let name = world
        .firms
        .get(p)
        .map(|f| f.name.clone())
        .unwrap_or_default();
    match state.tracking.warned_week {
        None => {
            if recent.len() >= periods && recent.iter().all(|n| *n < 0) {
                state.tracking.warned_week = Some(week);
                state.tracking.board_confidence = Some(false);
                let text = if holds_majority(world, p) {
                    majority_warning(&name, periods as u32 * PERIOD_WEEKS)
                } else {
                    format!(
                        "The board of {name} has lost confidence after {} weeks of losses. A profitable four-week period within {} weeks restores it; otherwise the board removes you.",
                        periods as u32 * PERIOD_WEEKS,
                        state.rules.board_warning_weeks
                    )
                };
                push_event(state, week, "board_warning", text.clone(), 30_000_000_000);
                alert(world, "career.board_warning", text);
                world.pending_pause = Some("career_board_warning".into());
            }
        }
        Some(warned) => {
            if last_profitable {
                state.tracking.warned_week = None;
                state.tracking.board_confidence = Some(true);
                let text = format!("The board of {name} is satisfied again: the losses stopped.");
                push_event(state, week, "board_restored", text.clone(), 25_000_000_000);
                alert(world, "career.board_restored", text);
            } else if week >= warned + state.rules.board_warning_weeks && holds_majority(world, p) {
                // A board the player controls cannot remove the player: the warning stands
                // and is read again, and the discipline is the market's.
                state.tracking.warned_week = Some(week);
                let text = majority_warning(&name, (week - warned) + periods as u32 * PERIOD_WEEKS);
                push_event(state, week, "board_warning", text.clone(), 10_000_000_000);
                alert(world, "career.board_warning", text);
            } else if week >= warned + state.rules.board_warning_weeks {
                let text = format!(
                    "The board of {name} removed you after {} more weeks of losses.",
                    week - warned
                );
                end(world, state, EndKind::Removed, text);
            }
        }
    }
}

fn quarter(world: &mut World, state: &mut CareerState) {
    let r = state.rules.clone();
    let lines = [
        r.dominant_bronze_bps,
        r.dominant_silver_bps,
        r.dominant_gold_bps,
    ];
    let lots = std::mem::take(&mut state.tracking.quarter_lots);
    let producers = std::mem::take(&mut state.tracking.quarter_producers);
    let mut best: Option<(u8, i64)> = None;
    for (industry, (ours, all)) in &lots {
        if *all <= 0 {
            continue;
        }
        let makers = producers.get(industry).map_or(0, |p| p.len() as u32);
        // A thin industry is not a contest: its share counts as nothing this quarter.
        let share = if makers < r.dominant_min_producers {
            0
        } else {
            (*ours as i128 * 10_000 / *all as i128) as i64
        };
        if best.is_none_or(|(_, s)| share > s) {
            best = Some((*industry, share));
        }
        let streak = state.tracking.streaks.entry(*industry).or_insert([0; 3]);
        for (i, line) in lines.iter().enumerate() {
            streak[i] = if share >= *line { streak[i] + 1 } else { 0 };
        }
        for (i, medal) in [Medal::Bronze, Medal::Silver, Medal::Gold]
            .into_iter()
            .enumerate()
        {
            if streak[i] >= r.dominant_quarters && medal > state.tracking.dominant_best {
                state.tracking.dominant_best = medal;
                state.tracking.dominant_industry = Some(*industry);
                state.tracking.dominant_share_bps = share;
            }
        }
    }
    let medal = standing(world, state);
    if let Some(ambition) = state.ambition {
        let p = progress(world, state);
        let detail = if p.next.is_empty() {
            p.detail
        } else {
            format!("{} {}", p.detail, p.next)
        };
        let text = format!(
            "Career progress ({}): {} Standing: {}.",
            ambition.key(),
            detail,
            medal.key()
        );
        alert(world, "career.progress", text);
        if ambition == Ambition::Dominant && state.tracking.dominant_best == Medal::Gold {
            let industry = state.tracking.dominant_industry.unwrap_or(0);
            let sentence = format!(
                "Your group made more than {} percent of the corridor's {} for {} quarters in a row.",
                r.dominant_gold_bps / 100,
                industry_name(industry),
                r.dominant_quarters
            );
            end(world, state, EndKind::Won, sentence);
        }
    }
}

fn industry_name(industry: u8) -> String {
    crate::state::INDUSTRY_NAMES
        .get(industry as usize)
        .copied()
        .unwrap_or("business")
        .to_lowercase()
}

fn year_line(world: &World, state: &CareerState, year: u32) -> YearLine {
    let league = company_league(world);
    let primary = live_primary(world);
    let (worth, wealth_rank, _) = wealth_league(world);
    YearLine {
        year,
        week: world.tick.0,
        rank: primary.and_then(|p| rank_in(&league, p)),
        league: league.len() as u32,
        company_value_cents: primary
            .and_then(|p| world.firms.get(p))
            .map(company_value)
            .unwrap_or(0),
        net_worth_cents: worth,
        wealth_rank,
        medal: standing(world, state),
    }
}

fn closing_sentence(world: &World, state: &CareerState, medal: Medal) -> String {
    let years = state.years;
    let Some(ambition) = state.ambition else {
        return format!("Your {years} years are up.");
    };
    let league = company_league(world);
    let rank = live_primary(world).and_then(|p| rank_in(&league, p));
    let fact = match ambition {
        Ambition::Richest => {
            let (worth, _, _) = wealth_league(world);
            format!(
                "your net worth of {} is {} times the {} you started with",
                money(worth),
                times(growth_bps(worth, state.start_worth_cents)),
                money(state.start_worth_cents)
            )
        }
        Ambition::Biggest => match rank {
            Some(r) => format!("your company ranked {r} of {} by value", league.len()),
            None => "you ran no company in the league".into(),
        },
        Ambition::Dominant => {
            if state.tracking.dominant_best == Medal::None {
                "your group never held a leading share of any industry for a year".into()
            } else {
                format!(
                    "your group's best hold was {}.{} percent of {}",
                    state.tracking.dominant_share_bps / 100,
                    (state.tracking.dominant_share_bps % 100) / 10,
                    industry_name(state.tracking.dominant_industry.unwrap_or(0))
                )
            }
        }
        Ambition::Survivor => match rank {
            Some(r) => format!(
                "your company is still standing, ranked {r} of {}",
                league.len()
            ),
            None => "you no longer run a solvent company".into(),
        },
    };
    format!(
        "After {years} years {fact}: {}.",
        match medal {
            Medal::Gold => "gold",
            Medal::Silver => "silver",
            Medal::Bronze => "bronze",
            Medal::None => "no medal",
        }
    )
}

fn end(world: &mut World, state: &mut CareerState, kind: EndKind, sentence: String) {
    let week = world.tick.0;
    let medal = match kind {
        EndKind::Won => Medal::Gold,
        EndKind::Completed => standing(world, state),
        _ => Medal::None,
    };
    // A last line for the year in progress.
    let elapsed = week.saturating_sub(state.start_week);
    if state.years_log.last().is_none_or(|l| l.week != week) {
        let line = year_line(world, state, elapsed.div_ceil(YEAR_WEEKS));
        state.years_log.push(line);
    }
    push_event(state, week, kind.key(), sentence.clone(), i64::MAX / 4);
    alert(world, "career.ended", sentence.clone());
    state.ending = Some(Ending {
        week,
        kind,
        medal,
        sentence,
    });
    world.pending_pause = Some("career_end".into());
}

/// The ten events that mattered (content count), by weight, then shown in date order.
pub fn key_events(state: &CareerState) -> Vec<CareerEvent> {
    let mut all = state.events.clone();
    all.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.week.cmp(&b.week)));
    // One of each raid kind's biggest, then the rest by weight.
    let mut out: Vec<CareerEvent> = Vec::new();
    let mut seen_raid: BTreeSet<String> = BTreeSet::new();
    for e in &all {
        if e.key.starts_with("raid_") && !seen_raid.insert(e.key.clone()) {
            continue;
        }
        out.push(e.clone());
        if out.len() >= state.rules.events_kept as usize {
            break;
        }
    }
    out.sort_by_key(|e| e.week);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    fn world() -> (World, Dataset) {
        let ds = Dataset::load_scenario("great-lakes-2000").unwrap();
        let w = seed::build(&ds, 7, ScenarioKind::Founder, ScenarioMode::Sandbox);
        (w, ds)
    }

    #[test]
    fn helm_offers_come_from_the_band_in_different_sectors() {
        let (w, _) = world();
        let offers = helm_offers(&w);
        assert!(!offers.is_empty());
        let t = career_table();
        for o in &offers {
            assert!((t.helm.rank_from..=t.helm.rank_to).contains(&o.rank));
            assert!(o.price_cents > 0 && o.shares_needed > 0);
        }
    }

    #[test]
    fn taking_the_helm_pays_the_sellers_and_creates_nothing() {
        let (mut w, ds) = world();
        let offer = helm_offers(&w).remove(0);
        let outside_before: i64 = w.outside.investors.iter().map(|h| h.capital_cents).sum();
        let standard = w.investor.cash_cents;
        let setup = CareerSetup {
            start: Start::Helm,
            helm_firm: Some(offer.firm.0),
            ambition: Some(Ambition::Biggest),
            years: 10,
        };
        begin(&mut w, &ds, &setup).unwrap();
        let outside_after: i64 = w.outside.investors.iter().map(|h| h.capital_cents).sum();
        assert_eq!(outside_after - outside_before, offer.price_cents);
        let helm = w.career.as_ref().unwrap().helm.clone().unwrap();
        assert_eq!(helm.standard_fortune_cents, standard);
        // Cash after = fortune - price, exactly.
        assert_eq!(
            w.investor.cash_cents,
            helm.fortune_cents - offer.price_cents
        );
        assert_eq!(
            w.firms.get(offer.firm).unwrap().shares.controller(),
            Some(HolderId::Investor)
        );
        assert_eq!(w.investor.primary_group, Some(offer.firm));
        assert!(w.firms.get(offer.firm).unwrap().ceo.is_none());
    }

    #[test]
    fn a_pick_that_is_not_for_sale_is_refused() {
        let (mut w, ds) = world();
        let setup = CareerSetup {
            start: Start::Helm,
            helm_firm: Some(u64::MAX),
            ambition: None,
            years: 10,
        };
        assert!(begin(&mut w, &ds, &setup).is_err());
        assert!(w.career.is_none());
    }

    #[test]
    fn medals_by_rank_follow_the_thresholds() {
        let (mut w, ds) = world();
        let offer = helm_offers(&w).remove(0);
        begin(
            &mut w,
            &ds,
            &CareerSetup {
                start: Start::Helm,
                helm_firm: Some(offer.firm.0),
                ambition: Some(Ambition::Biggest),
                years: 5,
            },
        )
        .unwrap();
        let state = w.career.clone().unwrap();
        let league = company_league(&w);
        let rank = rank_in(&league, offer.firm).unwrap();
        let expect = if rank <= 1 {
            Medal::Gold
        } else if rank <= 3 {
            Medal::Silver
        } else if rank <= 10 {
            Medal::Bronze
        } else {
            Medal::None
        };
        assert_eq!(standing(&w, &state), expect);
    }

    #[test]
    fn an_unfunded_founder_with_no_company_loses_after_the_grace() {
        let (mut w, ds) = world();
        begin(
            &mut w,
            &ds,
            &CareerSetup {
                ambition: Some(Ambition::Survivor),
                ..CareerSetup::default()
            },
        )
        .unwrap();
        w.investor.cash_cents = 1_000;
        let grace = career_table().found_grace_weeks;
        w.tick = cw_core::calendar::Tick(grace);
        weekly(&mut w, &ds);
        let ending = w.career.as_ref().unwrap().ending.clone().unwrap();
        assert_eq!(ending.kind, EndKind::NoCompany);
        assert_eq!(ending.medal, Medal::None);
        assert_eq!(w.pending_pause.as_deref(), Some("career_end"));
    }
    fn losing_helm(w: &mut World, ds: &Dataset) -> Handle {
        let offer = helm_offers(w).remove(0);
        begin(
            w,
            ds,
            &CareerSetup {
                start: Start::Helm,
                helm_firm: Some(offer.firm.0),
                ambition: Some(Ambition::Biggest),
                years: 10,
            },
        )
        .unwrap();
        let periods = career_table().board.loss_periods;
        let history: Vec<crate::state::PeriodClose> = (0..periods + 20)
            .map(|i| crate::state::PeriodClose {
                first_week: i * PERIOD_WEEKS,
                net_income: -100_000,
                ..Default::default()
            })
            .collect();
        w.firm_history.insert(offer.firm, history);
        offer.firm
    }

    fn read_board_at(w: &mut World, week: u32) {
        w.tick = cw_core::calendar::Tick(week);
        let mut state = w.career.take().unwrap();
        let led = w.investor.primary_group;
        board(w, &mut state, led, week);
        w.career = Some(state);
    }

    #[test]
    fn a_board_the_player_controls_warns_but_never_removes() {
        let (mut w, ds) = world();
        let firm = losing_helm(&mut w, &ds);
        assert!(holds_majority(&w, firm));
        let t = career_table();
        let first = t.board.grace_weeks.next_multiple_of(PERIOD_WEEKS);
        read_board_at(&mut w, first);
        let state = w.career.as_ref().unwrap();
        assert_eq!(state.tracking.warned_week, Some(first));
        assert!(
            state
                .events
                .last()
                .unwrap()
                .text
                .contains("cannot remove you")
        );
        // Long past the warning: still warned, never removed.
        for k in 1..=3 {
            read_board_at(
                &mut w,
                first + k * t.board.warning_weeks.next_multiple_of(PERIOD_WEEKS),
            );
        }
        let state = w.career.as_ref().unwrap();
        assert!(state.ending.is_none());
        assert!(state.tracking.warned_week.is_some());
    }

    #[test]
    fn a_minority_owner_is_removed_after_the_warning() {
        let (mut w, ds) = world();
        let firm = losing_helm(&mut w, &ds);
        // Sell down below half to an outside holder.
        let (ours, eligible) = player_votes(&w, firm);
        let sell = ours - eligible / 2 + 1;
        w.firms.get_mut(firm).unwrap().shares.transfer(
            HolderId::Investor,
            HolderId::Outside(0),
            sell,
            0,
        );
        assert!(!holds_majority(&w, firm));
        let t = career_table();
        let first = t.board.grace_weeks.next_multiple_of(PERIOD_WEEKS);
        read_board_at(&mut w, first);
        assert!(
            w.career
                .as_ref()
                .unwrap()
                .events
                .last()
                .unwrap()
                .text
                .contains("removes you")
        );
        read_board_at(
            &mut w,
            first + t.board.warning_weeks.next_multiple_of(PERIOD_WEEKS),
        );
        let ending = w.career.as_ref().unwrap().ending.clone().unwrap();
        assert_eq!(ending.kind, EndKind::Removed);
    }
    #[test]
    fn richest_is_the_growth_of_the_players_own_fortune() {
        let (mut w, ds) = world();
        begin(
            &mut w,
            &ds,
            &CareerSetup {
                ambition: Some(Ambition::Richest),
                years: 10,
                ..CareerSetup::default()
            },
        )
        .unwrap();
        let state = w.career.clone().unwrap();
        let start = state.start_worth_cents;
        assert_eq!(start, crate::economy::score::personal_net_worth(&w));
        assert_eq!(standing(&w, &state), Medal::None);
        let t = career_table().richest.for_years(10);
        let extra = |bps: i64| (start as i128 * (bps - 10_000) as i128 / 10_000) as i64;
        let base = w.investor.cash_cents;
        w.investor.cash_cents = base + extra(t.bronze_bps);
        assert_eq!(standing(&w, &state), Medal::Bronze);
        w.investor.cash_cents = base + extra(t.gold_bps);
        assert_eq!(standing(&w, &state), Medal::Gold);
        let p = progress(&w, &state);
        assert_eq!(p.medal, Medal::Gold);
        assert!(p.next.is_empty());
    }

    #[test]
    fn biggest_has_a_scale_for_each_start() {
        let t = career_table();
        let founder = Rules::from_table(t, Start::Founder, 10);
        let helm = Rules::from_table(t, Start::Helm, 10);
        assert_eq!(
            founder.rank_bronze_top_bps,
            t.biggest.founder.bronze_top_bps
        );
        assert_eq!(founder.rank_gold, t.biggest.founder.gold);
        assert_eq!(helm.rank_gold, t.biggest.helm.gold);
        assert!(founder.rank_gold > helm.rank_gold);
        let five = Rules::from_table(t, Start::Founder, 5);
        assert!(five.richest_bronze_bps < founder.richest_bronze_bps);
    }
}
