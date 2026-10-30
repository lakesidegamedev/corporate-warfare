//! Rival long plans (1.9): a raider, financier or integrator rival can run a campaign against
//! one target over several quarters, with the same commands and information a player has.
//!
//! * Building: buy the target's shares on the exchange in steps (`PlaceShareOrder`, at most half
//!   of the weekly executable float, bid no more than 2 percent over the mark, so price impact
//!   and the float limit every step). Crossing 5 percent of the votes is a public disclosure:
//!   a paper story and, when the target is the player's, a letter.
//! * Tender: at a quarter of the votes the rival launches an ordinary takeover bid for the rest
//!   (`StartDiligence`, the same path and the same 25 percent premium a rival bid always uses).
//! * Selling out: the plan is abandoned when the price runs away from where it started, the
//!   campaign runs past 78 weeks, the rival's cash runs short, the target falls under another
//!   holder's control (a white knight) or the tender fails. The rival sells its shares back
//!   into the float, step by step, and waits two years before another plan.
//!
//! The target answers with the tools everyone has: a buyback (a rival target does this itself
//! through `rival_defence`; it takes shares out of the float and lifts the price), a friendly
//! deal with another buyer, issuing shares, and raising guidance. There is no poison pill: the
//! board has no rule that dilutes or blocks a holder, and this pass does not add one.
//!
//! Nothing here moves money. Every purchase and sale is an exchange trade between real accounts,
//! and the rival's decisions are made by `rival_action`, which the AI turns into commands. The
//! plan itself is saved state (`World.plans`, trailer `PLAN`), hashed while not empty. Opening a
//! plan and moving it between stages is deterministic from the world, with no random draw.

use serde::{Deserialize, Serialize};

use crate::economy::{cast, deals, insolvency, ownership, rivalry};
use crate::state::{DealKind, DealState, HolderId, World};
use cw_core::ids::Handle;
use cw_data::{Dataset, Temperament};

/// Share of the votes at which a holding is public.
pub const DISCLOSE_BPS: i32 = 500;
/// Share of the votes at which the rival launches its tender.
pub const GOAL_BPS: i32 = 2_500;
/// Weeks a campaign may run before the rival gives up.
pub const MAX_WEEKS: u32 = 78;
/// Weeks a rival waits after a plan ends before it opens another.
pub const COOLDOWN_WEEKS: u32 = 104;
/// Weeks the rival waits for its tender to start before it gives up.
pub const TENDER_WAIT_WEEKS: u32 = 13;
/// Weeks it may take to sell the stake back.
pub const SELL_WEEKS: u32 = 52;
/// A step buys at most this share of the week's executable float, in basis points.
pub const STEP_FLOAT_BPS: i64 = 5_000;
/// A buy order bids at most this far over the mark.
pub const LIMIT_BPS: i64 = 200;
/// The first plan may open no earlier than this week.
pub const FIRST_WEEK: u32 = 26;
/// The rival looks for a target once in this many weeks.
const REVIEW_WEEKS: u32 = 13;
/// One plan for this many companies at most.
const FIRMS_PER_PLAN: usize = 8;
const EVENTS_KEPT: usize = 60;
const ENDED_KEPT: usize = 40;
const HISTORY_KEPT: usize = 156;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// Buying toward the goal.
    Building,
    /// The goal is reached; waiting for the bid to be funded and run.
    Tender,
    /// Giving up; selling the shares back.
    SellingOut,
}

impl Stage {
    pub const fn key(self) -> &'static str {
        match self {
            Stage::Building => "building",
            Stage::Tender => "tender",
            Stage::SellingOut => "selling_out",
        }
    }
}

/// Why a plan ended, or why it turned to selling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// The tender settled: the rival controls the target.
    Won,
    /// The price ran away from where the plan started.
    PriceRan,
    /// The campaign ran past its time limit.
    TimedOut,
    /// The rival's cash ran short.
    Cash,
    /// Another holder took control of the target.
    Controlled,
    /// The tender was withdrawn, expired or was rejected.
    TenderFailed,
    /// The target or the rival left the game.
    Gone,
}

impl Outcome {
    pub const fn key(self) -> &'static str {
        match self {
            Outcome::Won => "won",
            Outcome::PriceRan => "price_ran",
            Outcome::TimedOut => "timed_out",
            Outcome::Cash => "cash",
            Outcome::Controlled => "controlled",
            Outcome::TenderFailed => "tender_failed",
            Outcome::Gone => "gone",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub raider: Handle,
    pub target: Handle,
    /// Temperament code of the raider's chief when the plan opened.
    pub temperament: u8,
    pub stage: Stage,
    pub started: u32,
    pub stage_since: u32,
    /// The target's traded mark and fair estimate when the plan opened.
    pub entry_mark_cents: i64,
    pub entry_fair_cents: i64,
    /// What the rival already held when it started.
    pub base_held: i64,
    /// The week the stake crossed the disclosure line.
    pub disclosed: Option<u32>,
    /// The takeover deal, once the tender is launched.
    pub deal: Option<Handle>,
    /// Last observed stake, basis points of the votes.
    pub stake_bps: i32,
    /// Why the plan is selling out, when it is.
    pub reason: Option<Outcome>,
    /// Public stake history, one point a week from the disclosure on: (week, basis points).
    pub history: Vec<(u32, i32)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ended {
    pub raider: Handle,
    pub target: Handle,
    pub started: u32,
    pub ended: u32,
    pub outcome: Outcome,
    pub peak_bps: i32,
    pub disclosed: Option<u32>,
    pub history: Vec<(u32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    /// The stake crossed 5 percent of the votes.
    Disclosed,
    /// The stake crossed another five points.
    Grew,
    /// The rival launched its bid.
    Tender,
    /// The rival gave up and began to sell.
    Abandoned,
    /// The tender settled.
    Won,
    /// The rival sold down below the disclosure line.
    Dropped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEvent {
    pub week: u32,
    pub raider: Handle,
    pub target: Handle,
    pub kind: EventKind,
    pub stake_bps: i32,
    pub reason: Option<Outcome>,
}

/// Counts for telemetry. Never names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStats {
    pub opened: u32,
    pub disclosed: u32,
    pub tenders: u32,
    pub won: u32,
    pub abandoned: u32,
    pub bought_shares: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanState {
    pub plans: Vec<Plan>,
    pub ended: Vec<Ended>,
    /// The week before which a raider opens no new plan.
    pub cooldown: std::collections::BTreeMap<Handle, u32>,
    pub events: Vec<PlanEvent>,
    pub stats: PlanStats,
    pub migration: crate::economy::migration::MigrationState,
}

impl PlanState {
    pub fn is_empty(&self) -> bool {
        *self == PlanState::default()
    }

    pub fn plan_of(&self, raider: Handle) -> Option<&Plan> {
        self.plans.iter().find(|p| p.raider == raider)
    }

    pub fn plans_against(&self, target: Handle) -> impl Iterator<Item = &Plan> {
        self.plans.iter().filter(move |p| p.target == target)
    }
}

/// What a rival does this week for its plan, for the AI to send as ordinary commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Buy {
        target: Handle,
        qty: i64,
        limit_cents: i64,
    },
    Sell {
        target: Handle,
        qty: i64,
        limit_cents: i64,
    },
    Tender {
        target: Handle,
        price_cents: i64,
        shares: i64,
    },
}

/// Whether a temperament runs long plans.
pub fn plans_takeovers(t: Temperament) -> bool {
    matches!(
        t,
        Temperament::Raider | Temperament::Financier | Temperament::Integrator
    )
}

/// How far the target's mark may run over where the plan started before the rival gives up, in
/// basis points. A raider pays most for a prize; a financier least.
pub fn run_cap_bps(code: u8) -> i64 {
    match code {
        c if c == Temperament::Raider.code() => 5_000,
        c if c == Temperament::Integrator.code() => 4_000,
        _ => 3_000,
    }
}

/// Shares held by outside investors: the pool the exchange float is drawn from.
pub fn outside_held(world: &World, target: Handle) -> i64 {
    world
        .firms
        .get(target)
        .map(|f| {
            f.shares
                .holders
                .iter()
                .filter(|(h, _)| matches!(h, HolderId::Outside(_)))
                .map(|(_, lot)| lot.qty)
                .sum()
        })
        .unwrap_or(0)
}

/// The weekly executable float: two percent of the shares outside investors hold.
pub fn weekly_float(world: &World, target: Handle) -> i64 {
    (outside_held(world, target) * 2 / 100).max(0)
}

/// A holder's share of the votes, basis points.
pub fn stake_bps(world: &World, holder: HolderId, target: Handle) -> i32 {
    let Some(f) = world.firms.get(target) else {
        return 0;
    };
    let eligible = f.shares.eligible_votes();
    if eligible <= 0 {
        return 0;
    }
    (f.shares.qty(holder) as i128 * 10_000 / eligible as i128) as i32
}

fn reserve_of(world: &World, firm: Handle) -> i64 {
    insolvency::mandatory_weekly_outflow(world, firm).saturating_mul(8)
}

fn is_player_side(world: &World, firm: Handle) -> bool {
    ownership::ultimate_controller(world, firm) == Some(HolderId::Investor)
}

fn temperament_code(world: &World, ds: &Dataset, firm: Handle) -> Option<u8> {
    let t = cast::temperament(world, ds, firm)?;
    plans_takeovers(t).then(|| t.code())
}

/// What it costs a rival to reach the goal stake and then take the rest by tender, in cents.
fn campaign_cost(world: &World, raider: Handle, target: Handle) -> i64 {
    let Some(t) = world.firms.get(target) else {
        return i64::MAX;
    };
    let eligible = t.shares.eligible_votes();
    let held = t.shares.qty(HolderId::Firm(raider));
    let mark = t.shares.mark_cents.max(1);
    let goal = (eligible * GOAL_BPS as i64 / 10_000 - held).max(0);
    let rest = (eligible / 2 + 1 - held.max(eligible * GOAL_BPS as i64 / 10_000)).max(0);
    goal.saturating_mul(mark * (10_000 + LIMIT_BPS) / 10_000)
        .saturating_add(deals::escrow_required(mark * 125 / 100, rest))
}

/// A target worth a plan, scored; None when the rival may not or cannot go after it.
fn score_target(world: &World, ds: &Dataset, raider: Handle, target: Handle, code: u8) -> Option<i64> {
    let t = world.firms.get(target)?;
    let b = world.firms.get(raider)?;
    if target == raider
        || !t.operative()
        || !t.status.can_trade()
        || t.state_owned_since.is_some()
        || !t.shares.ipo_done
        || t.shares.mark_cents <= 0
        || ownership::same_group(world, raider, target)
        || ownership::would_create_cycle(world, raider, target)
        || world.plans.plans.iter().any(|p| p.target == target)
    {
        return None;
    }
    let eligible = t.shares.eligible_votes();
    if eligible <= 0 || t.shares.controller().is_some() {
        return None;
    }
    // Enough shares must be in outside hands for the exchange to deliver the goal.
    if outside_held(world, target) * 10_000 < eligible * (GOAL_BPS as i64 * 2) {
        return None;
    }
    let cost = campaign_cost(world, raider, target);
    let cash = b.book.cash() - reserve_of(world, raider);
    if cash <= 0 || cost > cash.saturating_mul(2) {
        return None;
    }
    let mark = t.shares.mark_cents.max(1);
    let discount = ((t.shares.fair_cents - mark) * 10_000 / mark).clamp(-3_000, 5_000);
    let grudge = rivalry::grudge_against(world, raider, HolderId::Firm(target)) as i64 / 4;
    let mut score = discount + grudge;
    if code == Temperament::Integrator.code() {
        let link = deals::vertical_share_bps(world, raider, target);
        if link > 0 {
            score += 2_000;
        } else {
            return None;
        }
    }
    if code == Temperament::Raider.code() {
        // A raider goes for firms that are hurting: a traded mark under the fair estimate.
        if discount < 500 {
            return None;
        }
    }
    if code == Temperament::Financier.code() && discount < 1_000 {
        return None;
    }
    let _ = ds;
    Some(score)
}

fn best_target(world: &World, ds: &Dataset, raider: Handle, code: u8) -> Option<Handle> {
    let mut best: Option<(i64, Handle)> = None;
    for (handle, _) in world.firms.iter() {
        if let Some(score) = score_target(world, ds, raider, handle, code)
            && best.is_none_or(|(s, h)| score > s || (score == s && handle < h))
        {
            best = Some((score, handle));
        }
    }
    best.map(|(_, h)| h)
}

fn push_event(world: &mut World, event: PlanEvent) {
    let events = &mut world.plans.events;
    events.push(event);
    if events.len() > EVENTS_KEPT {
        events.remove(0);
    }
}

fn log_move(world: &mut World, raider: Handle, text: String, class: &str) {
    let tick = world.tick.0;
    let moves = &mut world.rivalry.moves;
    moves.push(rivalry::RivalMove {
        week: tick,
        firm: raider,
        class: class.to_owned(),
        text,
    });
    if moves.len() > 80 {
        moves.remove(0);
    }
}

fn firm_name(world: &World, firm: Handle) -> String {
    world
        .firms
        .get(firm)
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "a former rival".to_owned())
}

/// Weekly: open plans, watch the stakes and move plans between stages. Runs after the exchange
/// and the deals have cleared, so it reads this week's fills.
pub fn weekly(world: &mut World, ds: &Dataset) {
    let now = world.tick.0;
    advance(world, now);
    open(world, ds, now);
}

fn open(world: &mut World, ds: &Dataset, now: u32) {
    if now < FIRST_WEEK {
        return;
    }
    let cap = (world.firms.len() / FIRMS_PER_PLAN).max(1);
    if world.plans.plans.len() >= cap {
        return;
    }
    let mut raiders: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(h, _)| h.slot() % REVIEW_WEEKS == now % REVIEW_WEEKS)
        .map(|(h, _)| h)
        .collect();
    raiders.sort_unstable();
    for raider in raiders {
        if world.plans.plans.len() >= cap {
            break;
        }
        let Some(firm) = world.firms.get(raider) else {
            continue;
        };
        if !firm.operative()
            || firm.state_owned_since.is_some()
            || is_player_side(world, raider)
            || world.plans.plan_of(raider).is_some()
            || world.plans.cooldown.get(&raider).is_some_and(|until| now < *until)
            || firm.shares.controller().is_some()
        {
            continue;
        }
        let Some(code) = temperament_code(world, ds, raider) else {
            continue;
        };
        let Some(target) = best_target(world, ds, raider, code) else {
            continue;
        };
        let t = world.firms.get(target).unwrap();
        let plan = Plan {
            raider,
            target,
            temperament: code,
            stage: Stage::Building,
            started: now,
            stage_since: now,
            entry_mark_cents: t.shares.mark_cents,
            entry_fair_cents: t.shares.fair_cents,
            base_held: t.shares.qty(HolderId::Firm(raider)),
            disclosed: None,
            deal: None,
            stake_bps: stake_bps(world, HolderId::Firm(raider), target),
            reason: None,
            history: Vec::new(),
        };
        world.plans.stats.opened += 1;
        world.plans.plans.push(plan);
    }
}

fn advance(world: &mut World, now: u32) {
    let mut finished: Vec<(usize, Outcome)> = Vec::new();
    for index in 0..world.plans.plans.len() {
        let plan = world.plans.plans[index].clone();
        let alive = |h: Handle| world.firms.get(h).is_some_and(|f| f.operative());
        if !alive(plan.raider) || !alive(plan.target) {
            finished.push((index, Outcome::Gone));
            continue;
        }
        let stake = stake_bps(world, HolderId::Firm(plan.raider), plan.target);
        let mark = world
            .firms
            .get(plan.target)
            .map(|f| f.shares.mark_cents)
            .unwrap_or(0);
        let mut event: Option<(EventKind, Option<Outcome>)> = None;
        let mut next_stage = plan.stage;
        let mut reason = plan.reason;
        let mut disclosed = plan.disclosed;
        match plan.stage {
            Stage::Building | Stage::Tender => {
                let controller = world
                    .firms
                    .get(plan.target)
                    .and_then(|f| f.shares.controller());
                let give_up = if controller.is_some_and(|c| c != HolderId::Firm(plan.raider)) {
                    Some(Outcome::Controlled)
                } else if !world
                    .firms
                    .get(plan.target)
                    .is_some_and(|f| f.status.can_trade())
                {
                    Some(Outcome::Gone)
                } else if plan.deal.is_none()
                    && mark > plan.entry_mark_cents * (10_000 + run_cap_bps(plan.temperament)) / 10_000
                {
                    Some(Outcome::PriceRan)
                } else if now.saturating_sub(plan.started) > MAX_WEEKS && plan.deal.is_none() {
                    Some(Outcome::TimedOut)
                } else if plan.deal.is_none()
                    && world
                        .firms
                        .get(plan.raider)
                        .is_some_and(|f| f.book.cash() < reserve_of(world, plan.raider) / 2)
                {
                    Some(Outcome::Cash)
                } else if plan.stage == Stage::Tender
                    && plan.deal.is_none()
                    && now.saturating_sub(plan.stage_since) > TENDER_WAIT_WEEKS
                {
                    Some(Outcome::TimedOut)
                } else {
                    None
                };
                if let Some(why) = give_up {
                    next_stage = Stage::SellingOut;
                    reason = Some(why);
                    event = Some((EventKind::Abandoned, Some(why)));
                } else if plan.stage == Stage::Building && stake >= GOAL_BPS {
                    next_stage = Stage::Tender;
                } else if plan.stage == Stage::Tender && plan.deal.is_none() {
                    // A tender the rival launched shows up as an open bid between the two.
                    if let Some(handle) = find_bid(world, plan.raider, plan.target) {
                        world.plans.plans[index].deal = Some(handle);
                        world.plans.stats.tenders += 1;
                        event = Some((EventKind::Tender, None));
                    }
                }
                if let Some(handle) = world.plans.plans[index].deal
                    && let Some(deal) = world.deals.get(handle)
                    && deal.state.terminal()
                {
                    if deal.state == DealState::Settled {
                        finished.push((index, Outcome::Won));
                        world.plans.stats.won += 1;
                        event = Some((EventKind::Won, Some(Outcome::Won)));
                    } else {
                        next_stage = Stage::SellingOut;
                        reason = Some(Outcome::TenderFailed);
                        event = Some((EventKind::Abandoned, Some(Outcome::TenderFailed)));
                    }
                }
            }
            Stage::SellingOut => {
                let held = world
                    .firms
                    .get(plan.target)
                    .map(|f| f.shares.qty(HolderId::Firm(plan.raider)))
                    .unwrap_or(0);
                if held <= 0 || now.saturating_sub(plan.stage_since) > SELL_WEEKS {
                    finished.push((index, plan.reason.unwrap_or(Outcome::TimedOut)));
                }
            }
        }
        // The stake is public from the line on, and its history with it.
        let mut grew = false;
        if disclosed.is_none() && stake >= DISCLOSE_BPS {
            disclosed = Some(now);
            world.plans.stats.disclosed += 1;
            event = event.or(Some((EventKind::Disclosed, None)));
            let text = format!(
                "Disclosed a {} percent stake in {}.",
                stake / 100,
                firm_name(world, plan.target)
            );
            log_move(world, plan.raider, text, "stake");
        } else if disclosed.is_some() && stake / 500 > plan.stake_bps / 500 && stake / 500 >= 2 {
            grew = true;
        }
        let dropped = disclosed.is_some()
            && plan.stake_bps >= DISCLOSE_BPS
            && stake < DISCLOSE_BPS
            && plan.stage == Stage::SellingOut;
        if dropped {
            event = Some((EventKind::Dropped, None));
        }
        if grew && event.is_none() {
            event = Some((EventKind::Grew, None));
        }
        {
            let p = &mut world.plans.plans[index];
            p.stake_bps = stake;
            if p.stage != next_stage {
                p.stage_since = now;
            }
            p.stage = next_stage;
            p.reason = reason;
            p.disclosed = disclosed;
            if disclosed.is_some() && (stake >= DISCLOSE_BPS || !p.history.is_empty()) {
                p.history.push((now, stake));
                if p.history.len() > HISTORY_KEPT {
                    p.history.remove(0);
                }
            }
        }
        if let Some((kind, why)) = event {
            let text = match kind {
                EventKind::Tender => Some(format!("Bid for {}.", firm_name(world, plan.target))),
                EventKind::Abandoned => Some(format!(
                    "Gave up on {} and is selling its stake.",
                    firm_name(world, plan.target)
                )),
                EventKind::Won => Some(format!("Took control of {}.", firm_name(world, plan.target))),
                _ => None,
            };
            if let Some(text) = text {
                log_move(world, plan.raider, text, "stake");
            }
            if kind == EventKind::Abandoned {
                world.plans.stats.abandoned += 1;
            }
            push_event(
                world,
                PlanEvent {
                    week: now,
                    raider: plan.raider,
                    target: plan.target,
                    kind,
                    stake_bps: stake,
                    reason: why,
                },
            );
        }
    }
    // Close finished plans, highest index first.
    finished.sort_by_key(|(i, _)| std::cmp::Reverse(*i));
    finished.dedup_by_key(|(i, _)| *i);
    for (index, outcome) in finished {
        let p = world.plans.plans.remove(index);
        world
            .plans
            .cooldown
            .insert(p.raider, now + COOLDOWN_WEEKS);
        let peak = p.history.iter().map(|(_, b)| *b).max().unwrap_or(p.stake_bps);
        world.plans.ended.push(Ended {
            raider: p.raider,
            target: p.target,
            started: p.started,
            ended: now,
            outcome,
            peak_bps: peak.max(p.stake_bps),
            disclosed: p.disclosed,
            history: p.history,
        });
        if world.plans.ended.len() > ENDED_KEPT {
            world.plans.ended.remove(0);
        }
    }
}

fn find_bid(world: &World, raider: Handle, target: Handle) -> Option<Handle> {
    world
        .deals
        .iter()
        .filter(|(_, d)| {
            d.buyer == raider
                && d.target == target
                && d.kind == DealKind::SharePurchase
                && !d.state.terminal()
        })
        .map(|(h, _)| h)
        .min()
}

/// The rival's move for its plan this week, if any. Pure: the AI sends it as a command and the
/// usual checks apply.
pub fn rival_action(world: &World, ds: &Dataset, firm: Handle) -> Option<Action> {
    let plan = world.plans.plan_of(firm)?;
    let me = world.firms.get(firm)?;
    let t = world.firms.get(plan.target)?;
    let mark = t.shares.mark_cents.max(1);
    let eligible = t.shares.eligible_votes();
    let held = t.shares.qty(HolderId::Firm(firm));
    let float = weekly_float(world, plan.target);
    let _ = ds;
    match plan.stage {
        Stage::Building => {
            let goal = eligible * GOAL_BPS as i64 / 10_000;
            let limit = mark * (10_000 + LIMIT_BPS) / 10_000;
            let room = (float * STEP_FLOAT_BPS / 10_000).max(0);
            let spare = (me.book.cash() - reserve_of(world, firm)).max(0);
            let qty = room.min((goal - held).max(0)).min(spare / limit.max(1));
            (qty > 0).then_some(Action::Buy {
                target: plan.target,
                qty,
                limit_cents: limit,
            })
        }
        Stage::Tender => {
            if plan.deal.is_some() {
                return None;
            }
            let price = mark + mark * 25 / 100;
            let wanted = (eligible / 2 + 1 - held).max(0);
            if wanted == 0 {
                return None;
            }
            let escrow = deals::escrow_required(price, wanted);
            let diligence = price.saturating_mul(wanted) / 200;
            if me.book.cash() < reserve_of(world, firm) + escrow + diligence {
                return None;
            }
            Some(Action::Tender {
                target: plan.target,
                price_cents: price,
                shares: wanted,
            })
        }
        Stage::SellingOut => {
            if held <= 0 {
                return None;
            }
            let qty = held.min((float * STEP_FLOAT_BPS / 10_000).max(1));
            Some(Action::Sell {
                target: plan.target,
                qty,
                limit_cents: (mark * 98 / 100).max(1),
            })
        }
    }
}

/// A target's own answer to a disclosed hostile stake: buy back shares, which takes them out of
/// the float and lifts the price. Returns the quantity. The target never lets a buyback push the
/// rival over the control line, and keeps a cash cushion.
pub fn rival_defence(world: &World, firm: Handle) -> Option<i64> {
    let plan = world
        .plans
        .plans_against(firm)
        .find(|p| p.disclosed.is_some() && p.stage != Stage::SellingOut && p.stake_bps >= 1_000)?;
    let me = world.firms.get(firm)?;
    if world.tick.0 % 4 != firm.slot() % 4 {
        return None;
    }
    let eligible = me.shares.eligible_votes();
    let held = me.shares.qty(HolderId::Firm(plan.raider));
    let qty = (eligible / 100).max(1);
    let mark = me.shares.mark_cents.max(1);
    if (held * 10_000) / (eligible - qty).max(1) >= 4_000 {
        return None;
    }
    let cost = qty.saturating_mul(mark);
    (me.book.cash() > reserve_of(world, firm) + cost * 2).then_some(qty)
}

/// A stake in a company that is public: its holder, size, when it became public and its history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disclosed {
    pub holder: Handle,
    pub bps: i32,
    pub since: Option<u32>,
    pub history: Vec<(u32, i32)>,
    /// A plan is running (or ran) with this stake; false for an ordinary holding.
    pub campaign: bool,
}

/// Every company holding at least 5 percent of `target`, strongest first. Only what the filing
/// shows: the size, the date and the weekly history since.
pub fn disclosed_stakes(world: &World, target: Handle) -> Vec<Disclosed> {
    let Some(t) = world.firms.get(target) else {
        return Vec::new();
    };
    let mut out: Vec<Disclosed> = Vec::new();
    for (holder, lot) in &t.shares.holders {
        let HolderId::Firm(h) = holder else { continue };
        if lot.qty <= 0 {
            continue;
        }
        let bps = stake_bps(world, *holder, target);
        if bps < DISCLOSE_BPS {
            continue;
        }
        let plan = world
            .plans
            .plans
            .iter()
            .find(|p| p.raider == *h && p.target == target && p.disclosed.is_some());
        out.push(Disclosed {
            holder: *h,
            bps,
            since: plan.and_then(|p| p.disclosed),
            history: plan.map(|p| p.history.clone()).unwrap_or_default(),
            campaign: plan.is_some(),
        });
    }
    out.sort_by(|a, b| b.bps.cmp(&a.bps).then(a.holder.cmp(&b.holder)));
    out
}

/// Weeks over which "growing" is judged.
const TREND_WEEKS: u32 = 13;

/// The public trend of a stake in `target` held by `raider`: change in basis points over the
/// last thirteen weeks, from the disclosed history only.
pub fn public_trend(world: &World, raider: Handle, target: Handle) -> Option<(i32, i32)> {
    let plan = world
        .plans
        .plans
        .iter()
        .find(|p| p.raider == raider && p.target == target && p.disclosed.is_some())?;
    let (_, now) = *plan.history.last()?;
    let week = plan.history.last()?.0;
    let then = plan
        .history
        .iter()
        .rev()
        .find(|(w, _)| week.saturating_sub(*w) >= TREND_WEEKS)
        .or_else(|| plan.history.first())
        .map(|(_, b)| *b)?;
    Some((now, now - then))
}

/// One line for a rival's dossier from what the filings and the public record show; never the
/// rival's hidden intent. None when nothing has been signalled.
pub fn likely_next_move(world: &World, rival: Handle) -> Option<String> {
    // A bid that is open is public.
    for (_, d) in world.deals.iter() {
        if d.buyer == rival
            && d.kind == DealKind::SharePurchase
            && !d.state.terminal()
            && !ownership::same_group(world, d.buyer, d.target)
        {
            return Some(format!(
                "Its offer for {} is open. Expect it to settle or lapse within weeks.",
                firm_name(world, d.target)
            ));
        }
    }
    // A filed stake and which way it is going.
    let mut best: Option<(i32, Handle)> = None;
    for plan in world.plans.plans.iter().filter(|p| p.raider == rival && p.disclosed.is_some()) {
        if best.is_none_or(|(b, _)| plan.stake_bps > b) {
            best = Some((plan.stake_bps, plan.target));
        }
    }
    if let Some((bps, target)) = best {
        let who = firm_name(world, target);
        let (now, change) = public_trend(world, rival, target).unwrap_or((bps, 0));
        let percent = now / 100;
        return Some(if change < -50 {
            format!("Selling down its {percent} percent stake in {who}.")
        } else if now >= GOAL_BPS - 500 {
            format!("Holds {percent} percent of {who} and is close to a bid. Expect a takeover offer soon.")
        } else if change > 50 {
            format!(
                "Building a stake in {who}, now {percent} percent and rising. A bid usually follows once it nears a quarter."
            )
        } else {
            format!("Holds {percent} percent of {who} and is holding steady.")
        });
    }
    // A campaign that just ended is on the record too.
    let now = world.tick.0;
    world
        .plans
        .ended
        .iter()
        .rev()
        .find(|e| e.raider == rival && e.disclosed.is_some() && now.saturating_sub(e.ended) < 52)
        .map(|e| match e.outcome {
            Outcome::Won => format!("Took control of {} recently. Expect it to turn to the next target.", firm_name(world, e.target)),
            _ => format!("Backed away from {} recently. Another campaign is not likely before next year.", firm_name(world, e.target)),
        })
}
