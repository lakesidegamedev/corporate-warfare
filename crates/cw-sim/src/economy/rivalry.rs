//! Rival memory: grudges and alliances between groups, and the log of what rivals did.
//!
//! A group that is raided, locked out of a lane or bid for against its will holds a grudge
//! against the group that did it. Joint venture partners and the two sides of a friendly deal
//! become allies. Both fade with time; a venture keeps its partners' alliance alive while it
//! runs. Memory is held per group (the ultimate controller), so a subsidiary shares its parent's
//! feelings. The player's group is the investor and never holds a grudge, but rivals hold them
//! against it. The rival AI reads this table; nothing here chooses a move.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::economy::ownership;
use crate::state::{HolderId, World};
use cw_core::ids::Handle;

/// Highest grudge or alliance strength.
pub const MAX_SCORE: i32 = 10_000;
/// Weekly fade of a grudge, basis points of its strength, and a flat step on top.
const GRUDGE_DECAY_BPS: i32 = 100;
/// Weekly fade of an alliance. Friends are forgotten more slowly than enemies are forgiven.
const ALLY_DECAY_BPS: i32 = 60;
/// Strength below which a memory is dropped.
const FLOOR: i32 = 150;
/// Strength a running joint venture holds its partners' alliance at.
const VENTURE_HOLD: i32 = 4_000;
const MAX_ENTRIES: usize = 96;
const MOVES_KEPT: usize = 80;
/// Strength at which a grudge or alliance counts as one for display and for the rival AI.
pub const ACTIVE: i32 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cause {
    Raid,
    FailedRaid,
    LaneLockout,
    HostileBid,
    Venture,
    Deal,
}

impl Cause {
    pub const fn key(self) -> &'static str {
        match self {
            Cause::Raid => "raid",
            Cause::FailedRaid => "failed_raid",
            Cause::LaneLockout => "lane_lockout",
            Cause::HostileBid => "hostile_bid",
            Cause::Venture => "venture",
            Cause::Deal => "deal",
        }
    }

    pub const fn phrase(self) -> &'static str {
        match self {
            Cause::Raid => "took plants from them",
            Cause::FailedRaid => "tried to raid them",
            Cause::LaneLockout => "locked them out of a lane",
            Cause::HostileBid => "made a hostile bid for them",
            Cause::Venture => "shares a venture with them",
            Cause::Deal => "closed a deal with them",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    pub score: i32,
    pub cause: Cause,
    pub last: u32,
}

/// One thing a rival did, for its dossier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalMove {
    pub week: u32,
    pub firm: Handle,
    pub class: String,
    pub text: String,
}

/// Counts for telemetry. Never names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalryStats {
    pub grudges_formed: u32,
    pub alliances_formed: u32,
    /// Moves aimed at a group the mover holds a grudge against.
    pub grudge_moves: u32,
    /// Moves by the mover's temperament key.
    pub moves_by_temperament: BTreeMap<String, u32>,
    /// Moves by class key.
    pub moves_by_class: BTreeMap<String, u32>,
    /// Moves by temperament and class, keyed "temperament/class".
    #[serde(default)]
    pub moves_by_pair: BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalryState {
    /// What a group (by its controlling firm) holds against another holder.
    pub grudges: BTreeMap<(Handle, HolderId), Memory>,
    /// Friendly ties, keyed by the ordered pair of holders.
    pub alliances: BTreeMap<(HolderId, HolderId), Memory>,
    pub moves: Vec<RivalMove>,
    pub stats: RivalryStats,
}

/// The group a firm belongs to, as a holder: the investor for the player's companies, otherwise
/// the firm at the top of the ownership chain.
pub fn root(world: &World, firm: Handle) -> HolderId {
    let mut current = firm;
    let mut hops = 0;
    loop {
        let Some(controller) = world.firms.get(current).and_then(|f| f.shares.controller()) else {
            return HolderId::Firm(current);
        };
        match controller {
            HolderId::Firm(parent) if parent != current && hops < 16 => {
                current = parent;
                hops += 1;
            }
            HolderId::Investor => return HolderId::Investor,
            _ => return HolderId::Firm(current),
        }
    }
}

fn pair(a: HolderId, b: HolderId) -> (HolderId, HolderId) {
    if a <= b { (a, b) } else { (b, a) }
}

/// `victim` was wronged by `offender`. Only a rival group holds a grudge.
pub fn note_grudge(world: &mut World, victim: Handle, offender: Handle, amount: i32, cause: Cause) {
    let HolderId::Firm(holder) = root(world, victim) else {
        return;
    };
    let target = root(world, offender);
    if target == HolderId::Firm(holder) || amount <= 0 {
        return;
    }
    let tick = world.tick.0;
    let state = &mut world.rivalry;
    let fresh = !state.grudges.contains_key(&(holder, target));
    let entry = state.grudges.entry((holder, target)).or_insert(Memory {
        score: 0,
        cause,
        last: tick,
    });
    if amount >= entry.score / 2 {
        entry.cause = cause;
    }
    entry.score = (entry.score + amount).min(MAX_SCORE);
    entry.last = tick;
    if fresh {
        state.stats.grudges_formed += 1;
    }
    // Betrayal cools an alliance.
    if let Some(ally) = state
        .alliances
        .get_mut(&pair(HolderId::Firm(holder), target))
    {
        ally.score /= 2;
    }
    trim(&mut state.grudges);
}

/// Two groups did something friendly together.
pub fn note_alliance(world: &mut World, a: Handle, b: Handle, amount: i32, cause: Cause) {
    let (ra, rb) = (root(world, a), root(world, b));
    if ra == rb || amount <= 0 {
        return;
    }
    // At least one side must be a rival for the tie to mean anything.
    if !matches!(ra, HolderId::Firm(_)) && !matches!(rb, HolderId::Firm(_)) {
        return;
    }
    let tick = world.tick.0;
    let state = &mut world.rivalry;
    let key = pair(ra, rb);
    let fresh = !state.alliances.contains_key(&key);
    let entry = state.alliances.entry(key).or_insert(Memory {
        score: 0,
        cause,
        last: tick,
    });
    entry.score = (entry.score + amount).min(MAX_SCORE);
    entry.cause = cause;
    entry.last = tick;
    if fresh {
        state.stats.alliances_formed += 1;
    }
    // Friends forgive.
    for (holder, other) in [(ra, rb), (rb, ra)] {
        if let HolderId::Firm(h) = holder
            && let Some(g) = state.grudges.get_mut(&(h, other))
        {
            g.score /= 2;
        }
    }
    trim(&mut state.alliances);
}

fn trim<K: Ord + Clone>(map: &mut BTreeMap<K, Memory>) {
    while map.len() > MAX_ENTRIES {
        let weakest = map
            .iter()
            .min_by_key(|(k, m)| (m.score, m.last, (*k).clone()))
            .map(|(k, _)| k.clone());
        match weakest {
            Some(k) => {
                map.remove(&k);
            }
            None => break,
        }
    }
}

/// How strongly the group of `rival` holds a grudge against the group of `target`.
pub fn grudge(world: &World, rival: Handle, target: Handle) -> i32 {
    let HolderId::Firm(holder) = root(world, rival) else {
        return 0;
    };
    grudge_against(world, holder, root(world, target))
}

pub fn grudge_against(world: &World, holder: Handle, target: HolderId) -> i32 {
    world
        .rivalry
        .grudges
        .get(&(holder, target))
        .map(|m| m.score)
        .unwrap_or(0)
}

/// How strong the tie between two firms' groups is.
pub fn alliance(world: &World, a: Handle, b: Handle) -> i32 {
    alliance_between(world, root(world, a), root(world, b))
}

pub fn alliance_between(world: &World, a: HolderId, b: HolderId) -> i32 {
    world
        .rivalry
        .alliances
        .get(&pair(a, b))
        .map(|m| m.score)
        .unwrap_or(0)
}

/// A rival's active grudges, strongest first.
pub fn grudges_of(world: &World, firm: Handle) -> Vec<(HolderId, Memory)> {
    let HolderId::Firm(holder) = root(world, firm) else {
        return Vec::new();
    };
    let mut out: Vec<(HolderId, Memory)> = world
        .rivalry
        .grudges
        .iter()
        .filter(|((h, _), m)| *h == holder && m.score >= ACTIVE)
        .map(|((_, t), m)| (*t, *m))
        .collect();
    out.sort_by(|a, b| b.1.score.cmp(&a.1.score).then(a.0.cmp(&b.0)));
    out
}

/// A rival's active allies, strongest first.
pub fn allies_of(world: &World, firm: Handle) -> Vec<(HolderId, Memory)> {
    let me = root(world, firm);
    let mut out: Vec<(HolderId, Memory)> = world
        .rivalry
        .alliances
        .iter()
        .filter(|((a, b), m)| (*a == me || *b == me) && m.score >= ACTIVE)
        .map(|((a, b), m)| (if *a == me { *b } else { *a }, *m))
        .collect();
    out.sort_by(|a, b| b.1.score.cmp(&a.1.score).then(a.0.cmp(&b.0)));
    out
}

/// Weekly: fade memories, keep running ventures' partners friendly and drop the dead.
pub fn weekly(world: &mut World) {
    let ventures: Vec<(Handle, Handle)> = world
        .structure
        .jvs
        .iter()
        .filter(|j| world.firms.get(j.firm).is_some_and(|f| f.operative()))
        .map(|j| (j.a, j.b))
        .collect();
    let live = |world: &World, h: HolderId| match h {
        HolderId::Firm(f) => world.firms.get(f).is_some_and(|x| x.operative()),
        _ => true,
    };
    let dead_g: Vec<(Handle, HolderId)> = world
        .rivalry
        .grudges
        .keys()
        .filter(|(h, t)| !live(world, HolderId::Firm(*h)) || !live(world, *t))
        .copied()
        .collect();
    let dead_a: Vec<(HolderId, HolderId)> = world
        .rivalry
        .alliances
        .keys()
        .filter(|(a, b)| !live(world, *a) || !live(world, *b))
        .copied()
        .collect();
    let state = &mut world.rivalry;
    for key in dead_g {
        state.grudges.remove(&key);
    }
    for key in dead_a {
        state.alliances.remove(&key);
    }
    for m in state.grudges.values_mut() {
        m.score = m.score - m.score * GRUDGE_DECAY_BPS / 10_000 - 1;
    }
    for m in state.alliances.values_mut() {
        m.score = m.score - m.score * ALLY_DECAY_BPS / 10_000 - 1;
    }
    state.grudges.retain(|_, m| m.score >= FLOOR);
    state.alliances.retain(|_, m| m.score >= FLOOR);
    for (a, b) in ventures {
        let (ra, rb) = (root(world, a), root(world, b));
        if ra == rb {
            continue;
        }
        let key = pair(ra, rb);
        let tick = world.tick.0;
        match world.rivalry.alliances.get_mut(&key) {
            Some(m) => {
                m.score = m.score.max(VENTURE_HOLD);
                m.last = tick;
            }
            None => {
                note_alliance(world, a, b, VENTURE_HOLD, Cause::Venture);
            }
        }
    }
}

/// Record something a rival did.
pub fn record_move(
    world: &mut World,
    firm: Handle,
    temperament: &str,
    class: &str,
    text: Option<String>,
    against_grudge: bool,
) {
    let week = world.tick.0;
    let state = &mut world.rivalry;
    *state
        .stats
        .moves_by_temperament
        .entry(temperament.to_owned())
        .or_default() += 1;
    *state
        .stats
        .moves_by_class
        .entry(class.to_owned())
        .or_default() += 1;
    *state
        .stats
        .moves_by_pair
        .entry(format!("{temperament}/{class}"))
        .or_default() += 1;
    if against_grudge {
        state.stats.grudge_moves += 1;
    }
    if let Some(text) = text {
        state.moves.push(RivalMove {
            week,
            firm,
            class: class.to_owned(),
            text,
        });
        let excess = state.moves.len().saturating_sub(MOVES_KEPT);
        state.moves.drain(..excess);
    }
}

// ---------------------------------------------------------------------------------------------
// Event hooks, called from the rules that create the events
// ---------------------------------------------------------------------------------------------

/// A raid resolved. The defender remembers; a taken plant is not forgiven quickly.
pub fn on_raid(world: &mut World, record: &crate::state::RaidRecord) {
    let (amount, cause) = if record.success {
        (3_000 + 600 * record.blocks as i32, Cause::Raid)
    } else {
        (1_500, Cause::FailedRaid)
    };
    note_grudge(world, record.defender, record.attacker, amount, cause);
}

/// A firm reserved lane capacity: other groups shipping on a lane that is filling up take it
/// as a lockout.
pub fn on_lane_reserved(world: &mut World, firm: Handle, key: crate::economy::logistics::LaneKey) {
    let Some(lane) = world.logistics.lanes.get(&key) else {
        return;
    };
    if lane.fill_bps() < 6_000 {
        return;
    }
    let total: i64 = lane.users.iter().map(|(_, lots)| *lots as i64).sum();
    let victims: Vec<(Handle, i32)> = lane
        .users
        .iter()
        .filter(|(f, lots)| *lots > 0 && !ownership::same_group(world, *f, firm))
        .map(|(f, lots)| (*f, *lots))
        .collect();
    for (victim, lots) in victims.into_iter().take(4) {
        let share = (lots as i64 * 10_000 / total.max(1)) as i32;
        note_grudge(world, victim, firm, 600 + share / 5, Cause::LaneLockout);
    }
}

/// A bid the target did not ask for.
pub fn on_hostile_bid(world: &mut World, buyer: Handle, target: Handle) {
    note_grudge(world, target, buyer, 2_500, Cause::HostileBid);
}

/// A joint venture formed.
pub fn on_venture(world: &mut World, a: Handle, b: Handle) {
    note_alliance(world, a, b, 5_000, Cause::Venture);
}

/// A friendly deal closed between the buyer and a seller of shares or assets.
pub fn on_deal(world: &mut World, buyer: Handle, seller: HolderId) {
    let seller_firm = match seller {
        HolderId::Firm(f) => f,
        HolderId::Investor => {
            // The player sold to a rival: tie the rival to the player's group.
            let Some(own) = world.investor.primary_group else {
                return;
            };
            own
        }
        _ => return,
    };
    note_alliance(world, buyer, seller_firm, 2_500, Cause::Deal);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_data::Dataset;

    fn world() -> World {
        let ds = Dataset::load().unwrap();
        seed::build(&ds, 3, ScenarioKind::Guided, ScenarioMode::Sandbox)
    }

    fn two_rivals(w: &World) -> (Handle, Handle) {
        let r: Vec<Handle> = w
            .firms
            .handles_sorted()
            .into_iter()
            .filter(|h| w.firms.get(*h).is_some_and(|f| f.archetype.is_some()))
            .collect();
        (r[0], r[1])
    }

    #[test]
    fn a_wronged_rival_holds_a_grudge_that_fades() {
        let mut w = world();
        let (a, b) = two_rivals(&w);
        note_grudge(&mut w, a, b, 4_000, Cause::Raid);
        assert_eq!(grudge(&w, a, b), 4_000);
        assert_eq!(grudge(&w, b, a), 0, "the offender holds nothing");
        assert_eq!(w.rivalry.stats.grudges_formed, 1);
        let mut last = 4_000;
        for _ in 0..52 {
            weekly(&mut w);
            let now = grudge(&w, a, b);
            assert!(now < last, "it fades every week");
            last = now;
        }
        assert!(
            last < 2_500 && last > 1_000,
            "about half gone in a year: {last}"
        );
        for _ in 0..400 {
            weekly(&mut w);
        }
        assert_eq!(grudge(&w, a, b), 0, "forgotten in the end");
        assert!(w.rivalry.grudges.is_empty());
    }

    #[test]
    fn the_investor_is_hated_but_never_hates() {
        let mut w = world();
        let (a, _) = two_rivals(&w);
        let player = w.investor.primary_group.expect("player company");
        note_grudge(&mut w, player, a, 5_000, Cause::Raid);
        assert!(w.rivalry.grudges.is_empty(), "the player holds no grudge");
        note_grudge(&mut w, a, player, 5_000, Cause::Raid);
        assert_eq!(grudge(&w, a, player), 5_000);
        assert_eq!(
            grudge_against(&w, a, HolderId::Investor),
            5_000,
            "held against the investor who runs the company"
        );
    }

    #[test]
    fn alliances_form_cool_a_grudge_and_break_on_betrayal() {
        let mut w = world();
        let (a, b) = two_rivals(&w);
        note_grudge(&mut w, a, b, 4_000, Cause::HostileBid);
        note_alliance(&mut w, a, b, 5_000, Cause::Venture);
        assert_eq!(alliance(&w, a, b), 5_000);
        assert_eq!(alliance(&w, b, a), 5_000, "symmetric");
        assert_eq!(grudge(&w, a, b), 2_000, "friends forgive");
        note_grudge(&mut w, a, b, 3_000, Cause::Raid);
        assert_eq!(alliance(&w, a, b), 2_500, "betrayal cools it");
        assert_eq!(w.rivalry.stats.alliances_formed, 1);
    }

    #[test]
    fn memory_round_trips_through_a_save() {
        let mut w = world();
        let (a, b) = two_rivals(&w);
        note_grudge(&mut w, a, b, 4_000, Cause::Raid);
        note_alliance(&mut w, b, a, 3_000, Cause::Deal);
        record_move(
            &mut w,
            a,
            "raider",
            "raid",
            Some("Raided someone.".into()),
            true,
        );
        let bytes = postcard::to_allocvec(&w.rivalry).unwrap();
        let back: RivalryState = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back, w.rivalry);
        assert_eq!(back.stats.grudge_moves, 1);
        assert_eq!(back.moves.len(), 1);
    }
}
