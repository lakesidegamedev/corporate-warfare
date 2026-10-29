//! Executives (1.9): three hireable posts in every company, each moving one real number.
//!
//! * Chief financial officer: the credit spread on loan and bond quotes (15 basis points per level,
//!   0.75 points at level 5) and a little interest on cash (4 basis points a year per level).
//! * Chief operating officer: build lead time (4 percent shorter per level, 20 percent at level 5)
//!   and the ceiling a fully staffed plant may run above its rating (0.8 points per level).
//! * Logistics chief: the premium outside carriers charge over the owners' empty-lane rate (5 percent
//!   less per level, 25 percent at level 5) and the fee for reserving lane capacity (1 percent per level).
//!
//! Candidates come from the generated people in `content/cast.json` (the extras, with their
//! temperaments and portraits). A pool of them is drawn when a world starts to play, each with a
//! post they suit and a level from 1 to 5. A company hires with `HireExecutive` (a fee of four
//! weeks of pay, then weekly pay that scales with level and with the size of the company),
//! `DismissExecutive` (severance) or `PoachExecutive` (an offer to a rival's executive).
//! Contracts run two years and then renew or the executive leaves. Rivals use the same commands
//! (`rival_pick`, `rival_poach`). Two missed board mandates in a row in the player's company
//! bring in a chief executive appointed by the board, the same turnover rule rivals have.
//!
//! The state lives outside the postcard payload (`World.executives`, save trailer `EXEC`) and is
//! folded into the canonical hash while it is not empty, like the chapter state. No decision here
//! draws from a shared random stream: every draw comes from a stream named by the world seed, the
//! firm and the week.

use serde::{Deserialize, Serialize};

use crate::state::{FirmStatus, World};
use cw_core::error::{DomainError, DomainResult};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::rng::Rng;
use cw_data::cast::extras;
use cw_data::{Dataset, Temperament};

const TAG_EXEC: u64 = 0x4558_4543_5554_4956;
const TAG_RENEW: u64 = 0x4558_4543_5245_4e57;

/// Weeks a contract runs.
pub const CONTRACT_WEEKS: u32 = 104;
/// Weeks of pay paid up front to a new hire, and to a poached one.
pub const SIGNING_WEEKS: i64 = 4;
/// Weeks a person who left is out of the pool.
pub const AWAY_WEEKS: u32 = 26;
/// Candidates drawn for each post when a world starts.
const POOL_PER_POST: usize = 8;
/// A post is topped up when fewer than this many people are available for it.
const POOL_LOW: usize = 3;
const EVENTS_KEPT: usize = 60;
/// Weeks of pay a firm keeps in cash after hiring.
pub const CASH_COVER_WEEKS: i64 = 26;
/// Chance, in basis points, that an executive whose effect pays stays at the end of a contract.
const RENEW_BPS: i32 = 8_500;
/// Weeks between a rival's looks at its executive posts.
const AI_REVIEW_WEEKS: u32 = 4;
/// Weeks a firm waits after a poaching attempt.
pub const POACH_COOLDOWN: u32 = 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Post {
    Cfo,
    Coo,
    Logistics,
}

impl Post {
    pub const ALL: [Post; 3] = [Post::Cfo, Post::Coo, Post::Logistics];

    pub const fn key(self) -> &'static str {
        match self {
            Post::Cfo => "cfo",
            Post::Coo => "coo",
            Post::Logistics => "logistics",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Post::Cfo => "Chief financial officer",
            Post::Coo => "Chief operating officer",
            Post::Logistics => "Logistics chief",
        }
    }

    pub const fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Option<Post> {
        Post::ALL.get(code as usize).copied()
    }
}

/// A person who can hold a post. `extra` is an index into the generated people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u16,
    pub extra: u16,
    pub post: Post,
    pub level: u8,
    /// The first week they can be hired (after leaving a company they are away for a while).
    pub available_from: u32,
}

/// Someone holding a post at a company.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Executive {
    pub firm: Handle,
    pub post: Post,
    pub person: u16,
    pub since: u32,
    pub expires: u32,
    pub pay_cents: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Hired,
    /// Left at the end of a contract or because pay was missed.
    Left,
    Dismissed,
    Renewed,
    /// Moved to `other` on a better offer; `firm` is the old company.
    Poached,
    /// The board named a chief executive for the company; `person` is a cast index.
    BoardChief,
}

/// One change, for papers, letters and dossiers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecEvent {
    pub week: u32,
    pub kind: EventKind,
    pub firm: Handle,
    pub other: Option<Handle>,
    pub post: Post,
    pub person: u16,
    /// Pay after the change, or severance for a dismissal.
    pub cents: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecState {
    pub seeded: bool,
    pub people: Vec<Candidate>,
    pub posts: Vec<Executive>,
    pub events: Vec<ExecEvent>,
    /// The chapter in which the board last named a chief executive for the company.
    pub board_chapter: Vec<(Handle, u32)>,
    /// The week each company last made a poaching offer.
    pub last_offer: Vec<(Handle, u32)>,
}

impl ExecState {
    pub fn is_empty(&self) -> bool {
        *self == ExecState::default()
    }

    pub fn person(&self, id: u16) -> Option<&Candidate> {
        self.people.iter().find(|c| c.id == id)
    }

    pub fn holder(&self, firm: Handle, post: Post) -> Option<&Executive> {
        self.posts
            .iter()
            .find(|e| e.firm == firm && e.post == post)
    }

    pub fn of_firm(&self, firm: Handle) -> impl Iterator<Item = &Executive> {
        self.posts.iter().filter(move |e| e.firm == firm)
    }

    fn push_event(&mut self, event: ExecEvent) {
        self.events.push(event);
        if self.events.len() > EVENTS_KEPT {
            self.events.remove(0);
        }
    }
}

// ----- effects -----

/// The level a company's executive holds, or zero for an empty post.
pub fn level(world: &World, firm: Handle, post: Post) -> u8 {
    if world.executives.posts.is_empty() {
        return 0;
    }
    world
        .executives
        .holder(firm, post)
        .and_then(|e| world.executives.person(e.person))
        .map(|c| c.level)
        .unwrap_or(0)
}

/// Basis points off the credit spread of loan and bond quotes.
pub const fn spread_cut_for(level: u8) -> i32 {
    15 * level as i32
}

/// Annual basis points earned on cash.
pub const fn cash_yield_for(level: u8) -> i32 {
    4 * level as i32
}

/// Percent off build lead time.
pub const fn lead_cut_pct_for(level: u8) -> u32 {
    4 * level as u32
}

/// Basis points a fully staffed plant may run above its rating.
pub const fn ceiling_for(level: u8) -> i32 {
    80 * level as i32
}

/// Percent off the outside carriers' premium over the owners' empty-lane rate.
pub const fn premium_cut_pct_for(level: u8) -> i64 {
    5 * level as i64
}

/// Basis points off the fee for reserving lane capacity.
pub const fn lane_fee_cut_for(level: u8) -> i64 {
    100 * level as i64
}

/// A loan or bond spread after the chief financial officer's cut. Never below 25 basis points.
pub fn spread_after_cut(world: &World, firm: Handle, spread_bps: i32) -> i32 {
    let cut = spread_cut_for(level(world, firm, Post::Cfo));
    if cut == 0 {
        spread_bps
    } else {
        (spread_bps - cut).max(25.min(spread_bps))
    }
}

/// Build weeks after the chief operating officer's cut. Never below one week.
pub fn build_weeks(world: &World, firm: Handle, weeks: u32) -> u32 {
    let cut = lead_cut_pct_for(level(world, firm, Post::Coo));
    if cut == 0 {
        weeks
    } else {
        (weeks - weeks * cut / 100).max(1)
    }
}

/// The ceiling bonus for a plant of this firm, in basis points above full rating.
pub fn ceiling_bonus_bps(world: &World, firm: Handle) -> i32 {
    ceiling_for(level(world, firm, Post::Coo))
}

/// An outside carrier price multiplier after the logistics chief's cut on the premium.
pub fn outside_mult_bps(world: &World, ds: &Dataset, firm: Handle, mult_bps: i64) -> i64 {
    let cut = premium_cut_pct_for(level(world, firm, Post::Logistics));
    let floor = ds.logistics.empty_lane_bps;
    if cut == 0 || mult_bps <= floor {
        mult_bps
    } else {
        floor + (mult_bps - floor) * (100 - cut) / 100
    }
}

/// A lane reservation fee after the logistics chief's planning.
pub fn lane_fee_after_cut(world: &World, firm: Handle, fee_cents: i64) -> i64 {
    let cut = lane_fee_cut_for(level(world, firm, Post::Logistics));
    if cut == 0 {
        fee_cents
    } else {
        (fee_cents * (10_000 - cut) / 10_000).max(1)
    }
}

// ----- people and pay -----

/// The generated person behind a candidate.
pub fn extra_of(c: &Candidate) -> Option<&'static cw_data::cast::Extra> {
    extras().get(c.extra as usize)
}

pub fn temperament_of(c: &Candidate) -> Temperament {
    extra_of(c)
        .map(|e| e.temperament)
        .unwrap_or(Temperament::Cautious)
}

fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// What a company of this size pays for a level, per week, in cents. Pay grows with the
/// square root of total assets: a small firm pays a fraction of what a giant does.
pub fn pay_for(level: u8, assets_cents: i64) -> i64 {
    // Tenths of a dollar per week per square-root-dollar of assets.
    let k = match level {
        0 | 1 => 3,
        2 => 5,
        3 => 8,
        4 => 12,
        _ => 18,
    };
    let root = isqrt((assets_cents / 100).max(10_000));
    // k is in tenths of a dollar, root in whole dollars' square roots, result in cents.
    root * k * 10
}

/// Basis points on top of the going rate a person of this temperament asks for.
const fn ask_bps(t: Temperament) -> i32 {
    match t {
        Temperament::Raider | Temperament::Speculator => 10_800,
        Temperament::Financier => 10_300,
        Temperament::Builder | Temperament::Integrator => 10_000,
        Temperament::Cautious => 9_600,
    }
}

/// The weekly pay a candidate asks of a company, in cents.
pub fn ask_cents(world: &World, firm: Handle, c: &Candidate) -> i64 {
    let assets = world
        .firms
        .get(firm)
        .map(|f| f.book.total_assets())
        .unwrap_or(0);
    pay_for(c.level, assets) * ask_bps(temperament_of(c)) as i64 / 10_000
}

/// What dismissing an executive costs: a quarter of the pay left on the contract, at least four
/// weeks and at most twenty-six weeks of pay.
pub fn severance_cents(world: &World, e: &Executive) -> i64 {
    let left = e.expires.saturating_sub(world.tick.0) as i64;
    (e.pay_cents * left / 4).clamp(e.pay_cents * 4, e.pay_cents * 26)
}

/// Candidates a company could hire for a post right now, best level first.
pub fn candidates(world: &World, post: Post) -> Vec<&Candidate> {
    let hired: Vec<u16> = world.executives.posts.iter().map(|e| e.person).collect();
    let mut out: Vec<&Candidate> = world
        .executives
        .people
        .iter()
        .filter(|c| c.post == post && c.available_from <= world.tick.0 && !hired.contains(&c.id))
        .collect();
    out.sort_by_key(|c| (std::cmp::Reverse(c.level), c.id));
    out
}

/// What a level at a post is worth to a company over a year, in cents, from the company's own
/// figures: the spread on its debt and the interest on its cash; the extra output a better
/// ceiling gives; the premium it pays outside carriers plus the lane fees it pays.
pub fn yearly_value_cents(world: &World, ds: &Dataset, firm: Handle, post: Post, lvl: u8) -> i64 {
    let Some(f) = world.firms.get(firm) else {
        return 0;
    };
    match post {
        Post::Cfo => {
            let debt = f.book.debt().max(0) as i128;
            let cash = f.book.cash().max(0) as i128;
            ((debt * spread_cut_for(lvl) as i128 + cash * cash_yield_for(lvl) as i128) / 10_000)
                as i64
        }
        Post::Coo => {
            // Output gain on a year of the last week's revenue, less the half that goes to
            // inputs and wages, plus a fifth of a year's carrying cost on plants being built.
            let revenue = f
                .book
                .recent_weeks()
                .last()
                .map(|w| w.revenue)
                .unwrap_or(0)
                .max(0) as i128;
            let gain = revenue * 52 * ceiling_for(lvl) as i128 / 10_000 / 2;
            let building = f
                .facilities
                .iter()
                .filter_map(|h| world.facilities.get(*h))
                .filter(|p| matches!(p.status, crate::state::FacilityStatus::Building))
                .count() as i128;
            let carry = building * lead_cut_pct_for(lvl) as i128 * 5_000_000 / 100;
            (gain + carry) as i64
        }
        Post::Logistics => {
            let (outside, lanes) = freight_spend_cents(world, ds, firm);
            let premium = ds.logistics.outside_premium_bps - ds.logistics.empty_lane_bps;
            let share = if ds.logistics.outside_premium_bps > 0 {
                premium.max(0) as i128 * 10_000 / ds.logistics.outside_premium_bps as i128
            } else {
                0
            };
            let on_premium =
                outside as i128 * share / 10_000 * premium_cut_pct_for(lvl) as i128 / 100;
            let on_fees = lanes as i128 * lane_fee_cut_for(lvl) as i128 / 10_000;
            ((on_premium + on_fees) * 52) as i64
        }
    }
}

/// Weekly (outside carrier spend, lane reservation fees) of a firm, from the last closed week.
fn freight_spend_cents(world: &World, ds: &Dataset, firm: Handle) -> (i64, i64) {
    let summary = crate::economy::logistics::shipping_summary(world, ds, firm, None);
    (summary.outside_cents, summary.fee_cents)
}

// ----- commands -----

fn bad(expected: &'static str, actual: &'static str) -> DomainError {
    DomainError::WrongPhase { expected, actual }
}

fn ensure_operating(world: &World, firm: Handle) -> DomainResult<()> {
    match world.firms.get(firm) {
        Some(f) if matches!(f.status, FirmStatus::Operating | FirmStatus::PastDue { .. }) => Ok(()),
        _ => Err(bad("a company that is operating", "company not operating")),
    }
}

fn spend(world: &mut World, firm: Handle, amount: i64, memo: &str) -> DomainResult<()> {
    let cash = world
        .firms
        .get(firm)
        .map(|f| f.book.cash().max(0))
        .unwrap_or(0);
    if cash < amount {
        return Err(DomainError::InsufficientCash {
            entity: firm.0,
            required: amount,
            available: cash,
        });
    }
    pay_outside(world, firm, amount, memo);
    Ok(())
}

/// Cash leaves the company to the outside world, as pay does.
fn pay_outside(world: &mut World, firm: Handle, amount: i64, memo: &str) -> i64 {
    if amount <= 0 {
        return 0;
    }
    let cash = world
        .firms
        .get(firm)
        .map(|f| f.book.cash().max(0))
        .unwrap_or(0);
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

/// Hire a candidate into a vacant post.
pub fn hire(world: &mut World, firm: Handle, post: Post, person: u16) -> DomainResult<()> {
    ensure_operating(world, firm)?;
    if world.executives.holder(firm, post).is_some() {
        return Err(bad("an empty post", "post already filled"));
    }
    let tick = world.tick.0;
    let hired = world.executives.posts.iter().any(|e| e.person == person);
    let c = world
        .executives
        .person(person)
        .filter(|c| c.post == post && c.available_from <= tick && !hired)
        .cloned()
        .ok_or_else(|| bad("a candidate for this post", "no such candidate"))?;
    let pay = ask_cents(world, firm, &c);
    let fee = pay * SIGNING_WEEKS;
    let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
    if cash < fee + pay * CASH_COVER_WEEKS {
        return Err(DomainError::InsufficientCash {
            entity: firm.0,
            required: fee + pay * CASH_COVER_WEEKS,
            available: cash.max(0),
        });
    }
    spend(world, firm, fee, "executive signing fee")?;
    seat(world, firm, post, person, pay, EventKind::Hired, None);
    Ok(())
}

fn seat(
    world: &mut World,
    firm: Handle,
    post: Post,
    person: u16,
    pay: i64,
    kind: EventKind,
    from: Option<Handle>,
) {
    let tick = world.tick.0;
    world.executives.posts.push(Executive {
        firm,
        post,
        person,
        since: tick,
        expires: tick + CONTRACT_WEEKS,
        pay_cents: pay,
    });
    world.executives.posts.sort_by_key(|e| (e.firm, e.post));
    world.executives.push_event(ExecEvent {
        week: tick,
        kind,
        firm,
        other: from,
        post,
        person,
        cents: pay,
    });
}

/// Let an executive go and pay severance. The person is away for a while.
pub fn dismiss(world: &mut World, firm: Handle, post: Post) -> DomainResult<i64> {
    let e = world
        .executives
        .holder(firm, post)
        .cloned()
        .ok_or_else(|| bad("a filled post", "post is empty"))?;
    let severance = severance_cents(world, &e);
    spend(world, firm, severance, "executive severance")?;
    release(world, firm, post, EventKind::Dismissed, severance, None);
    Ok(severance)
}

fn release(
    world: &mut World,
    firm: Handle,
    post: Post,
    kind: EventKind,
    cents: i64,
    other: Option<Handle>,
) {
    let tick = world.tick.0;
    let Some(at) = world
        .executives
        .posts
        .iter()
        .position(|e| e.firm == firm && e.post == post)
    else {
        return;
    };
    let e = world.executives.posts.remove(at);
    if let Some(c) = world.executives.people.iter_mut().find(|c| c.id == e.person) {
        c.available_from = tick + AWAY_WEEKS;
    }
    world.executives.push_event(ExecEvent {
        week: tick,
        kind,
        firm,
        other,
        post,
        person: e.person,
        cents,
    });
}

/// How far above current pay an offer must go before a person of this temperament moves, in
/// basis points. Cautious people want a lot to leave; speculators and raiders follow money.
pub const fn poach_hurdle_bps(t: Temperament) -> i32 {
    match t {
        Temperament::Speculator => 800,
        Temperament::Raider => 1_000,
        Temperament::Financier => 1_500,
        Temperament::Builder => 2_000,
        Temperament::Integrator => 2_500,
        Temperament::Cautious => 3_500,
    }
}

/// The least weekly pay that moves a rival's executive.
pub fn poach_price_cents(world: &World, from: Handle, post: Post) -> Option<i64> {
    let e = world.executives.holder(from, post)?;
    let c = world.executives.person(e.person)?;
    Some(e.pay_cents * (10_000 + poach_hurdle_bps(temperament_of(c))) as i64 / 10_000)
}

/// Offer a rival's executive a raise to move. The offer is accepted when it meets the temperament's
/// hurdle over current pay; the company then pays a signing fee of eight weeks of the new pay.
pub fn poach(
    world: &mut World,
    firm: Handle,
    from: Handle,
    post: Post,
    offer_cents: i64,
) -> DomainResult<()> {
    ensure_operating(world, firm)?;
    if firm == from {
        return Err(bad("another company's executive", "own executive"));
    }
    if world.executives.holder(firm, post).is_some() {
        return Err(bad("an empty post", "post already filled"));
    }
    let price = poach_price_cents(world, from, post)
        .ok_or_else(|| bad("a filled post at that company", "post is empty"))?;
    let tick = world.tick.0;
    if offer_cents <= 0 {
        return Err(bad("a pay offer", "no offer"));
    }
    set_last_offer(world, firm, tick);
    if offer_cents < price {
        return Err(bad("an offer the executive accepts", "offer refused"));
    }
    let fee = offer_cents * SIGNING_WEEKS * 2;
    let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
    if cash < fee + offer_cents * CASH_COVER_WEEKS {
        return Err(DomainError::InsufficientCash {
            entity: firm.0,
            required: fee + offer_cents * CASH_COVER_WEEKS,
            available: cash.max(0),
        });
    }
    spend(world, firm, fee, "executive signing fee")?;
    let person = world
        .executives
        .holder(from, post)
        .map(|e| e.person)
        .expect("checked above");
    let at = world
        .executives
        .posts
        .iter()
        .position(|e| e.firm == from && e.post == post)
        .expect("checked above");
    world.executives.posts.remove(at);
    world.executives.push_event(ExecEvent {
        week: tick,
        kind: EventKind::Poached,
        firm: from,
        other: Some(firm),
        post,
        person,
        cents: offer_cents,
    });
    seat(world, firm, post, person, offer_cents, EventKind::Hired, Some(from));
    Ok(())
}

fn set_last_offer(world: &mut World, firm: Handle, tick: u32) {
    match world.executives.last_offer.iter_mut().find(|(f, _)| *f == firm) {
        Some(entry) => entry.1 = tick,
        None => world.executives.last_offer.push((firm, tick)),
    }
}

pub fn last_offer(world: &World, firm: Handle) -> Option<u32> {
    world
        .executives
        .last_offer
        .iter()
        .find(|(f, _)| *f == firm)
        .map(|(_, w)| *w)
}

// ----- the weekly tick -----

/// Seed the pool from the generated people, deterministically from the world seed.
fn seed_pool(world: &mut World) {
    let pool = extras();
    if pool.is_empty() {
        world.executives.seeded = true;
        return;
    }
    let mut rng = Rng::stream(world.seed, TAG_EXEC);
    let mut taken: Vec<u16> = world.executives.people.iter().map(|c| c.extra).collect();
    for post in Post::ALL {
        for _ in 0..POOL_PER_POST {
            add_candidate(world, &mut rng, post, &mut taken);
        }
    }
    world.executives.seeded = true;
}

fn add_candidate(world: &mut World, rng: &mut Rng, post: Post, taken: &mut Vec<u16>) {
    let pool = extras();
    if taken.len() >= pool.len() {
        return;
    }
    let mut at = rng.uniform_below(pool.len() as u64) as u16;
    while taken.contains(&at) {
        at = (at + 1) % pool.len() as u16;
    }
    taken.push(at);
    // Levels one to five, middling ones commonest.
    let level = 1 + rng.weighted_index(&[30, 28, 22, 14, 6]) as u8;
    let id = world.executives.people.iter().map(|c| c.id + 1).max().unwrap_or(1);
    world.executives.people.push(Candidate {
        id,
        extra: at,
        post,
        level,
        available_from: 0,
    });
}

pub fn weekly(world: &mut World, dataset: &Dataset) {
    if !world.executives.seeded {
        seed_pool(world);
    }
    let tick = world.tick.0;
    pay_and_renew(world, dataset);
    cash_interest(world);
    if tick > 0 && tick.is_multiple_of(13) {
        top_up(world);
    }
    board_replaces_player_chief(world, dataset);
}

fn top_up(world: &mut World) {
    let tick = world.tick.0;
    let mut rng = Rng::entity_stream(world.seed, TAG_EXEC, Handle(tick as u64));
    let mut taken: Vec<u16> = world.executives.people.iter().map(|c| c.extra).collect();
    for post in Post::ALL {
        let open = candidates(world, post).len();
        if open < POOL_LOW {
            for _ in 0..2 {
                add_candidate(world, &mut rng, post, &mut taken);
            }
        }
    }
}

/// Weekly pay, then renewals at the end of a contract.
fn pay_and_renew(world: &mut World, dataset: &Dataset) {
    let tick = world.tick.0;
    let posts: Vec<Executive> = world.executives.posts.clone();
    for e in posts {
        let alive = world
            .firms
            .get(e.firm)
            .is_some_and(|f| !matches!(f.status, FirmStatus::Merged { .. } | FirmStatus::Liquidating));
        if !alive {
            release(world, e.firm, e.post, EventKind::Left, 0, None);
            continue;
        }
        let paid = pay_outside(world, e.firm, e.pay_cents, "executive pay");
        if paid < e.pay_cents {
            release(world, e.firm, e.post, EventKind::Left, 0, None);
            continue;
        }
        if tick >= e.expires {
            let Some(c) = world.executives.person(e.person).cloned() else {
                continue;
            };
            let value = yearly_value_cents(world, dataset, e.firm, e.post, c.level);
            let new_pay = ask_cents(world, e.firm, &c);
            let mut rng = Rng::entity_stream(
                world.seed,
                TAG_RENEW,
                Handle(e.firm.0 ^ ((tick as u64) << 24) ^ e.post.code() as u64),
            );
            let stays = value >= new_pay * 52 / 2 && rng.chance_bps(RENEW_BPS);
            if stays {
                if let Some(slot) = world
                    .executives
                    .posts
                    .iter_mut()
                    .find(|x| x.firm == e.firm && x.post == e.post)
                {
                    slot.since = tick;
                    slot.expires = tick + CONTRACT_WEEKS;
                    slot.pay_cents = new_pay;
                }
                world.executives.push_event(ExecEvent {
                    week: tick,
                    kind: EventKind::Renewed,
                    firm: e.firm,
                    other: None,
                    post: e.post,
                    person: e.person,
                    cents: new_pay,
                });
            } else {
                release(world, e.firm, e.post, EventKind::Left, 0, None);
            }
        }
    }
}

/// A chief financial officer earns a little interest on the company's cash, paid by the outside
/// world, at 4 basis points a year for each level.
fn cash_interest(world: &mut World) {
    let holders: Vec<(Handle, u8)> = world
        .executives
        .posts
        .iter()
        .filter(|e| e.post == Post::Cfo)
        .map(|e| (e.firm, level(world, e.firm, Post::Cfo)))
        .collect();
    for (firm, lvl) in holders {
        let cash = world
            .firms
            .get(firm)
            .map(|f| f.book.cash().max(0))
            .unwrap_or(0);
        let interest = cash * cash_yield_for(lvl) as i64 / (10_000 * 52);
        if interest <= 0 || world.outside.gateway_book.cash() < interest {
            continue;
        }
        if let Some(f) = world.firms.get_mut(firm) {
            let _ = f.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, interest, "treasury interest")
                    .credit(Account::InterestIncome, interest, "treasury interest"),
            );
        }
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, interest, "treasury interest")
                .debit(Account::Revenue, interest, "treasury interest"),
        );
    }
}

// ----- rivals -----

/// A rival's hire this week, by the same rule a player would apply: the vacant post whose
/// effect is worth the most over its pay, where the firm can afford the fee and the cover, and
/// the effect is at least one and a half times a year of pay. Cheaper levels are tried too.
pub fn rival_pick(world: &World, dataset: &Dataset, firm: Handle) -> Option<(Post, u16)> {
    if world.executives.people.is_empty() || !world.tick.0.is_multiple_of(AI_REVIEW_WEEKS) {
        return None;
    }
    let f = world.firms.get(firm)?;
    if !matches!(f.status, FirmStatus::Operating) {
        return None;
    }
    let cash = f.book.cash();
    let mut best: Option<(i64, Post, u16)> = None;
    for post in Post::ALL {
        if world.executives.holder(firm, post).is_some() {
            continue;
        }
        for c in candidates(world, post) {
            let ask = ask_cents(world, firm, c);
            if cash < ask * (SIGNING_WEEKS + CASH_COVER_WEEKS) {
                continue;
            }
            let value = yearly_value_cents(world, dataset, firm, post, c.level);
            let yearly_pay = ask * 52;
            if value * 2 < yearly_pay * 3 {
                continue;
            }
            let surplus = value - yearly_pay;
            if best.is_none_or(|(s, ..)| surplus > s) {
                best = Some((surplus, post, c.id));
            }
        }
    }
    best.map(|(_, post, id)| (post, id))
}

/// A rival's poaching offer: a raider or speculator led firm with a vacant post that no pool
/// candidate fills well, aimed at the best executive of a rival at the price that moves them.
pub fn rival_poach(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    temperament: Option<Temperament>,
) -> Option<(Handle, Post, i64)> {
    if !matches!(temperament, Some(Temperament::Raider | Temperament::Speculator))
        || !world.tick.0.is_multiple_of(AI_REVIEW_WEEKS)
    {
        return None;
    }
    if last_offer(world, firm).is_some_and(|w| w + POACH_COOLDOWN > world.tick.0) {
        return None;
    }
    let cash = world.firms.get(firm)?.book.cash();
    let mut best: Option<(i64, Handle, Post, i64)> = None;
    for e in &world.executives.posts {
        if e.firm == firm || world.executives.holder(firm, e.post).is_some() {
            continue;
        }
        let c = world.executives.person(e.person)?;
        if c.level < 3 {
            continue;
        }
        let price = poach_price_cents(world, e.firm, e.post)?;
        if cash < price * (SIGNING_WEEKS * 2 + CASH_COVER_WEEKS) {
            continue;
        }
        let value = yearly_value_cents(world, dataset, firm, e.post, c.level);
        if value * 2 < price * 52 * 3 {
            continue;
        }
        let surplus = value - price * 52;
        if best.is_none_or(|(s, ..)| surplus > s) {
            best = Some((surplus, e.firm, e.post, price));
        }
    }
    best.map(|(_, from, post, price)| (from, post, price))
}

// ----- the board and the player's chief executive -----

/// After two missed board mandates in a row in the player's company the board names a chief
/// executive, the same turnover a rival's board takes. The count starts again from that chapter.
fn board_replaces_player_chief(world: &mut World, dataset: &Dataset) {
    let Some(firm) = world.investor.primary_group else {
        return;
    };
    if dataset.cast.is_empty() {
        return;
    }
    let tick = world.tick.0;
    let Some(last) = world
        .chapters
        .log
        .iter()
        .rev()
        .find(|c| c.firm == Some(firm) && c.closed == tick && c.outcome.is_some())
    else {
        return;
    };
    if last.outcome != Some(crate::economy::chapters::Outcome::Missed) {
        return;
    }
    let since = board_chapter(world, firm);
    let counted: Vec<crate::economy::chapters::Outcome> = world
        .chapters
        .log
        .iter()
        .filter(|c| c.firm == Some(firm) && c.number > since)
        .filter_map(|c| c.outcome)
        .filter(|o| {
            !matches!(
                o,
                crate::economy::chapters::Outcome::Lapsed
                    | crate::economy::chapters::Outcome::Cancelled
            )
        })
        .collect();
    let missed_in_a_row = counted
        .iter()
        .rev()
        .take(2)
        .filter(|o| **o == crate::economy::chapters::Outcome::Missed)
        .count()
        == 2;
    if !missed_in_a_row {
        return;
    }
    let number = last.number;
    let scenario_id = world.cast.scenario_id.clone();
    let mut pool = crate::economy::cast::free_people(world, dataset, cw_data::Role::Ceo);
    pool.sort_by_key(|p| {
        (
            std::cmp::Reverse(dataset.cast[*p as usize].scenario == scenario_id),
            *p,
        )
    });
    let Some(person) = pool.first().copied() else {
        return;
    };
    if let Some(f) = world.firms.get_mut(firm) {
        f.ceo = Some(person);
    }
    world.cast.appointed.insert(firm, tick);
    match world
        .executives
        .board_chapter
        .iter_mut()
        .find(|(f, _)| *f == firm)
    {
        Some(entry) => entry.1 = number,
        None => world.executives.board_chapter.push((firm, number)),
    }
    world.executives.push_event(ExecEvent {
        week: tick,
        kind: EventKind::BoardChief,
        firm,
        other: None,
        post: Post::Cfo,
        person: person as u16,
        cents: 0,
    });
}

/// The chapter in which the board last named a chief executive for the company, or zero.
pub fn board_chapter(world: &World, firm: Handle) -> u32 {
    world
        .executives
        .board_chapter
        .iter()
        .find(|(f, _)| *f == firm)
        .map(|(_, n)| *n)
        .unwrap_or(0)
}

// ----- words for the screens, papers and letters -----

fn points(bps: i64) -> String {
    format!("{}.{:02}", bps / 100, bps % 100)
}

/// What a level at a post does, in plain words with the number.
pub fn effect_text(post: Post, level: u8) -> String {
    match post {
        Post::Cfo => format!(
            "Cuts the interest spread on loan and bond quotes by {} points and adds {} points a year on cash",
            points(spread_cut_for(level) as i64),
            points(cash_yield_for(level) as i64)
        ),
        Post::Coo => format!(
            "Cuts build time by {} percent and lets a full plant run {} points above its rating",
            lead_cut_pct_for(level),
            points(ceiling_for(level) as i64)
        ),
        Post::Logistics => format!(
            "Cuts the outside carriers' premium by {} percent and lane reservation fees by {} percent",
            premium_cut_pct_for(level),
            lane_fee_cut_for(level) / 100
        ),
    }
}

/// The generated person's name, or a placeholder if the content is missing.
pub fn name_of(c: &Candidate) -> String {
    extra_of(c).map(|e| e.name.clone()).unwrap_or_default()
}

/// Tests and screens: the person's id for the portrait atlas.
pub fn portrait_id(c: &Candidate) -> String {
    extra_of(c).map(|e| e.id.clone()).unwrap_or_default()
}
