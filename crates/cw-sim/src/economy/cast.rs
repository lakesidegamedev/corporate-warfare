//! The character cast in the economy: who chairs the central bank, leads the parties, governs
//! states and runs the rival firms.
//!
//! People come from `content/cast.json`. Posts are held by one person at a time, filled in a
//! fixed order from a stream seeded by the world, and a scenario's own people are preferred
//! over the rest. Rival firms get a named chief executive while there are people to spare;
//! the rest keep unnamed management on their archetype. After a long run of losses a rival's
//! board replaces its chief executive with someone free. A person's temperament is read from
//! their firm through [`temperament`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::state::{CentralBanker, FirmStatus, PoliticalNews, World};
use cw_core::ids::Handle;
use cw_core::rng::Rng;
use cw_data::{Archetype, Dataset, Role, Temperament};

/// A person's index in `Dataset::cast`.
pub type PersonId = u8;

const TAG_CAST: u64 = 0x4341_5354_5045_4f50;
const TAG_TURNOVER: u64 = 0x4341_5354_5455_524e;
/// People kept free of chief executive posts so a board always has someone to turn to.
const RESERVE: usize = 2;
/// State governors named at the start and after each election.
const GOVERNORS: usize = 3;
/// Four-week periods in a row of losses before a board loses patience.
pub const LOSS_PERIODS: usize = 6;
/// Weeks a chief executive must have served before a board acts.
pub const MIN_TENURE_WEEKS: u32 = 52;
/// Chance, per four-week review, that a board in that state replaces its chief executive.
const TURNOVER_BPS: i32 = 2_500;
/// Weeks a removed chief executive stays out of the pool.
pub const BENCH_WEEKS: u32 = 104;
const MOVES_KEPT: usize = 40;

/// One change at the top of a firm, for dossiers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastMove {
    pub week: u32,
    pub firm: Handle,
    pub person: PersonId,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastState {
    /// The scenario whose people fit best. Empty means no preference.
    pub scenario_id: String,
    /// Leader of each party: business first, labour second.
    pub leaders: [Option<PersonId>; 2],
    /// Governor of each named state, by state index in code order.
    pub governors: BTreeMap<u8, PersonId>,
    /// The week each firm's current chief executive started.
    pub appointed: BTreeMap<Handle, u32>,
    /// People removed from a post, and the week they become available again.
    pub benched: BTreeMap<PersonId, u32>,
    /// Chief executives replaced so far.
    pub turnovers: u32,
    pub moves: Vec<CastMove>,
}

fn fit(ds: &Dataset, scenario_id: &str, person: PersonId) -> i32 {
    if !scenario_id.is_empty() && ds.cast[person as usize].scenario == scenario_id {
        10
    } else {
        0
    }
}

/// How well a temperament suits a firm archetype: 2 best, 1 good, 0 neutral.
fn temperament_match(archetype: Archetype, temperament: Temperament) -> i32 {
    use Archetype::*;
    use Temperament::*;
    let (first, second) = match archetype {
        CostLeader => (Builder, Cautious),
        BrandBuilder => (Integrator, Builder),
        Consolidator => (Integrator, Financier),
        Technologist => (Speculator, Builder),
        Vulture => (Raider, Speculator),
    };
    if temperament == first {
        2
    } else if temperament == second {
        1
    } else {
        0
    }
}

/// The best candidate by score, ties drawn from the stream, or none.
fn best(
    candidates: &[PersonId],
    score: impl Fn(PersonId) -> i32,
    rng: &mut Rng,
) -> Option<PersonId> {
    let top = candidates.iter().map(|p| score(*p)).max()?;
    let tied: Vec<PersonId> = candidates
        .iter()
        .copied()
        .filter(|p| score(*p) == top)
        .collect();
    Some(tied[rng.uniform_below(tied.len() as u64) as usize])
}

/// Rival firms, in handle order. Rivals are the firms with an archetype.
fn rivals(world: &World) -> Vec<Handle> {
    world
        .firms
        .handles_sorted()
        .into_iter()
        .filter(|h| world.firms.get(*h).is_some_and(|f| f.archetype.is_some()))
        .collect()
}

/// Everyone holding a post right now.
fn busy(world: &World) -> Vec<PersonId> {
    let mut out: Vec<PersonId> = Vec::new();
    out.extend(world.macro_state.cycle.chair.person);
    out.extend(world.cast.leaders.iter().flatten());
    out.extend(world.cast.governors.values());
    for (_, firm) in world.firms.iter() {
        out.extend(firm.ceo);
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn free_people(world: &World, ds: &Dataset, role: Role) -> Vec<PersonId> {
    let held = busy(world);
    (0..ds.cast.len() as PersonId)
        .filter(|p| ds.cast[*p as usize].can(role))
        .filter(|p| !held.contains(p))
        .filter(|p| {
            world
                .cast
                .benched
                .get(p)
                .is_none_or(|until| *until <= world.tick.0)
        })
        .collect()
}

/// Name every post from the cast, starting from nothing. Deterministic for a world seed and a
/// scenario id; calling it again with another scenario id re-draws everything.
pub fn assign(world: &mut World, ds: &Dataset, scenario_id: &str) {
    if ds.cast.is_empty() {
        return;
    }
    let mut rng = Rng::stream(world.seed, TAG_CAST);
    world.cast = CastState {
        scenario_id: scenario_id.to_owned(),
        ..CastState::default()
    };
    for (_, firm) in world.firms.iter_mut() {
        firm.ceo = None;
    }
    world.macro_state.cycle.chair.person = None;

    // The central bank chair.
    let pool = free_people(world, ds, Role::Chair);
    if let Some(p) = best(&pool, |p| fit(ds, scenario_id, p), &mut rng) {
        let person = &ds.cast[p as usize];
        let chair = &mut world.macro_state.cycle.chair;
        chair.name = person.name.clone();
        chair.stance = person.stance.unwrap_or(0);
        chair.person = Some(p);
    }
    // Party leaders.
    for slot in 0..2 {
        let pool = free_people(world, ds, Role::Leader);
        let pick = best(&pool, |p| fit(ds, scenario_id, p), &mut rng);
        world.cast.leaders[slot] = pick;
    }
    draw_governors(world, ds, &mut rng);
    // Rival chief executives while there are people to spare.
    for firm in rivals(world) {
        let pool = free_people(world, ds, Role::Ceo);
        if pool.len() <= RESERVE {
            break;
        }
        let archetype = world.firms.get(firm).and_then(|f| f.archetype);
        let pick = best(
            &pool,
            |p| ceo_score(ds, scenario_id, p, archetype),
            &mut rng,
        );
        if let Some(f) = world.firms.get_mut(firm) {
            f.ceo = pick;
        }
        if pick.is_some() {
            world.cast.appointed.insert(firm, world.tick.0);
        }
    }
}

fn ceo_score(ds: &Dataset, scenario_id: &str, p: PersonId, archetype: Option<Archetype>) -> i32 {
    let fit = fit(ds, scenario_id, p);
    let style = archetype
        .map(|a| temperament_match(a, ds.cast[p as usize].temperament))
        .unwrap_or(0);
    fit + style * 5
}

/// Name governors: states with a native governor-capable person first, then the rest in code
/// order, up to [`GOVERNORS`].
fn draw_governors(world: &mut World, ds: &Dataset, rng: &mut Rng) {
    world.cast.governors.clear();
    let scenario_id = world.cast.scenario_id.clone();
    let mut order: Vec<usize> = (0..ds.states.len()).collect();
    let native = |state: usize| {
        ds.cast
            .iter()
            .any(|p| p.can(Role::Governor) && p.state == ds.states[state].code)
    };
    order.sort_by_key(|s| (!native(*s), *s));
    for state in order {
        if world.cast.governors.len() >= GOVERNORS {
            break;
        }
        let pool = free_people(world, ds, Role::Governor);
        let code = &ds.states[state].code;
        let pick = best(
            &pool,
            |p| {
                fit(ds, &scenario_id, p)
                    + if &ds.cast[p as usize].state == code {
                        20
                    } else {
                        0
                    }
            },
            rng,
        );
        if let Some(p) = pick {
            world.cast.governors.insert(state as u8, p);
        }
    }
}

/// A fresh governor line-up, called after each election.
pub fn redraw_governors(world: &mut World, ds: &Dataset) {
    if ds.cast.is_empty() {
        return;
    }
    let mut rng = Rng::entity_stream(world.seed, TAG_CAST, Handle(world.tick.0 as u64));
    draw_governors(world, ds, &mut rng);
}

/// The governing party's leader is the country's leader.
pub fn country_leader(world: &World) -> Option<PersonId> {
    world.cast.leaders[match world.politics.party {
        crate::state::Party::Business => 0,
        crate::state::Party::Labour => 1,
    }]
}

pub fn party_leader(world: &World, party: crate::state::Party) -> Option<PersonId> {
    world.cast.leaders[match party {
        crate::state::Party::Business => 0,
        crate::state::Party::Labour => 1,
    }]
}

/// A firm's chief executive.
pub fn ceo(world: &World, firm: Handle) -> Option<PersonId> {
    world.firms.get(firm).and_then(|f| f.ceo)
}

/// How a firm's chief executive plays, if it has a named one.
pub fn temperament(world: &World, ds: &Dataset, firm: Handle) -> Option<Temperament> {
    let p = ceo(world, firm)?;
    ds.cast.get(p as usize).map(|person| person.temperament)
}

/// A new central bank chair for a government's taste: a draw toward a stance, then the free
/// chair-capable person nearest it, preferring the scenario's own people. None when nobody is
/// free, in which case the incumbent stays.
pub fn pick_chair(world: &World, ds: &Dataset, desired_stance: i32) -> Option<PersonId> {
    let current = world.macro_state.cycle.chair.person;
    let pool: Vec<PersonId> = free_people(world, ds, Role::Chair)
        .into_iter()
        .filter(|p| Some(*p) != current)
        .collect();
    let scenario_id = &world.cast.scenario_id;
    let native: Vec<PersonId> = pool
        .iter()
        .copied()
        .filter(|p| fit(ds, scenario_id, *p) > 0)
        .collect();
    let pool = if native.is_empty() { pool } else { native };
    pool.into_iter().min_by_key(|p| {
        let stance = ds.cast[*p as usize].stance.unwrap_or(0);
        ((stance - desired_stance).abs(), *p)
    })
}

/// Apply a drawn chair to the bank.
pub fn seat_chair(ds: &Dataset, chair: &mut CentralBanker, person: PersonId) {
    let p = &ds.cast[person as usize];
    chair.name = p.name.clone();
    chair.stance = p.stance.unwrap_or(0);
    chair.person = Some(person);
}

/// Weekly: free the posts of dead firms and, every four weeks, let boards act on long losses.
pub fn weekly(world: &mut World, ds: &Dataset) {
    if ds.cast.is_empty() {
        return;
    }
    let week = world.tick.0;
    for firm in rivals(world) {
        let Some(f) = world.firms.get_mut(firm) else {
            continue;
        };
        if f.ceo.is_some()
            && matches!(
                f.status,
                FirmStatus::Merged { .. } | FirmStatus::Liquidating
            )
        {
            f.ceo = None;
            world.cast.appointed.remove(&firm);
        }
    }
    if week == 0 || week % crate::state::PERIOD_WEEKS != 0 {
        return;
    }
    for firm in rivals(world) {
        review(world, ds, firm);
    }
}

fn review(world: &mut World, ds: &Dataset, firm: Handle) {
    let week = world.tick.0;
    let Some(f) = world.firms.get(firm) else {
        return;
    };
    let Some(old) = f.ceo else {
        return;
    };
    if !matches!(f.status, FirmStatus::Operating | FirmStatus::PastDue { .. }) {
        return;
    }
    let since = world.cast.appointed.get(&firm).copied().unwrap_or(0);
    if week < since + MIN_TENURE_WEEKS {
        return;
    }
    let Some(history) = world.firm_history.get(&firm) else {
        return;
    };
    // Only periods after this chief executive started count against them.
    let recent: Vec<&crate::state::PeriodClose> = history
        .iter()
        .rev()
        .take_while(|p| p.first_week >= since)
        .take(LOSS_PERIODS)
        .collect();
    if recent.len() < LOSS_PERIODS || recent.iter().any(|p| p.net_income >= 0) {
        return;
    }
    let mut rng = Rng::entity_stream(
        world.seed,
        TAG_TURNOVER,
        Handle(firm.0 ^ (week as u64) << 40),
    );
    if !rng.chance_bps(TURNOVER_BPS) {
        return;
    }
    let archetype = f.archetype;
    let old_temperament = ds.cast[old as usize].temperament;
    let scenario_id = world.cast.scenario_id.clone();
    let pool: Vec<PersonId> = free_people(world, ds, Role::Ceo)
        .into_iter()
        .filter(|p| *p != old)
        .collect();
    // A board that sacked a chief executive wants a different approach as well as a better one.
    let Some(new) = best(
        &pool,
        |p| {
            ceo_score(ds, &scenario_id, p, archetype)
                + if ds.cast[p as usize].temperament != old_temperament {
                    3
                } else {
                    0
                }
        },
        &mut rng,
    ) else {
        return;
    };
    let name = world
        .firms
        .get(firm)
        .map(|f| f.name.clone())
        .unwrap_or_default();
    let text = format!(
        "{} replaces {} as chief executive of {} after {} weeks of losses.",
        ds.cast[new as usize].name,
        ds.cast[old as usize].name,
        name,
        LOSS_PERIODS as u32 * crate::state::PERIOD_WEEKS
    );
    if let Some(f) = world.firms.get_mut(firm) {
        f.ceo = Some(new);
    }
    world.cast.appointed.insert(firm, week);
    world.cast.benched.insert(old, week + BENCH_WEEKS);
    world.cast.turnovers += 1;
    world.cast.moves.push(CastMove {
        week,
        firm,
        person: new,
        text: text.clone(),
    });
    let excess = world.cast.moves.len().saturating_sub(MOVES_KEPT);
    world.cast.moves.drain(..excess);
    world.politics.news.push(PoliticalNews {
        week,
        text,
        person: Some(new),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    fn world(seed_value: u64, scenario: &str) -> (World, Dataset) {
        let ds = Dataset::load().unwrap();
        let mut w = seed::build(&ds, seed_value, ScenarioKind::Guided, ScenarioMode::Sandbox);
        assign(&mut w, &ds, scenario);
        (w, ds)
    }

    #[test]
    fn posts_are_filled_once_and_deterministically() {
        let (a, ds) = world(5, "great-lakes-2000");
        let (b, _) = world(5, "great-lakes-2000");
        assert_eq!(a.cast, b.cast);
        let held = busy(&a);
        let posts = a.macro_state.cycle.chair.person.iter().count()
            + a.cast.leaders.iter().flatten().count()
            + a.cast.governors.len()
            + a.firms.iter().filter(|(_, f)| f.ceo.is_some()).count();
        assert_eq!(held.len(), posts, "nobody holds two posts");
        let ceos = a.firms.iter().filter(|(_, f)| f.ceo.is_some()).count();
        assert_eq!(ceos, ds.cast.len() - 1 - 2 - GOVERNORS - RESERVE);
    }

    #[test]
    fn the_scenarios_own_people_are_preferred() {
        let (w, ds) = world(9, "texas-1981");
        let chair = w.macro_state.cycle.chair.person.unwrap();
        assert_eq!(ds.cast[chair as usize].scenario, "texas-1981");
        assert_eq!(w.macro_state.cycle.chair.name, ds.cast[chair as usize].name);
        assert_eq!(
            w.macro_state.cycle.chair.stance,
            ds.cast[chair as usize].stance.unwrap()
        );
        let (w, ds) = world(9, "ruhr-rhine-1991");
        let chair = w.macro_state.cycle.chair.person.unwrap();
        assert_eq!(ds.cast[chair as usize].scenario, "ruhr-rhine-1991");
    }

    #[test]
    fn an_unknown_scenario_falls_back_to_anyone() {
        let (w, _) = world(3, "not-a-scenario");
        assert!(w.macro_state.cycle.chair.person.is_some());
        assert!(w.cast.leaders.iter().all(|l| l.is_some()));
    }

    #[test]
    fn a_losing_board_replaces_its_chief_executive_and_the_old_one_is_benched() {
        let (mut w, ds) = world(7, "great-lakes-2000");
        let firm = w
            .firms
            .handles_sorted()
            .into_iter()
            .find(|h| w.firms.get(*h).is_some_and(|f| f.ceo.is_some()))
            .unwrap();
        let old = ceo(&w, firm).unwrap();
        let history: Vec<crate::state::PeriodClose> = (0..LOSS_PERIODS as u32)
            .map(|i| crate::state::PeriodClose {
                first_week: 1 + i * 4,
                weeks: 4,
                net_income: -1_000,
                ..Default::default()
            })
            .collect();
        w.firm_history.insert(firm, history);
        let mut replaced = false;
        for step in 0..40u32 {
            w.tick = cw_core::calendar::Tick(MIN_TENURE_WEEKS + 4 + step * 4);
            // Keep the losses recent enough to count.
            review(&mut w, &ds, firm);
            if ceo(&w, firm) != Some(old) {
                replaced = true;
                break;
            }
        }
        assert!(
            replaced,
            "a 25 percent review chance fires within forty reviews"
        );
        assert_eq!(w.cast.turnovers, 1);
        assert!(w.cast.benched.contains_key(&old));
        assert_eq!(w.cast.moves.len(), 1);
        assert_eq!(w.politics.news.last().unwrap().person, ceo(&w, firm));
    }

    #[test]
    fn profitable_firms_keep_their_chief_executive() {
        let (mut w, ds) = world(7, "great-lakes-2000");
        let firm = w
            .firms
            .handles_sorted()
            .into_iter()
            .find(|h| w.firms.get(*h).is_some_and(|f| f.ceo.is_some()))
            .unwrap();
        let old = ceo(&w, firm).unwrap();
        let history: Vec<crate::state::PeriodClose> = (0..LOSS_PERIODS as u32)
            .map(|i| crate::state::PeriodClose {
                first_week: 1 + i * 4,
                weeks: 4,
                net_income: if i == 3 { 1 } else { -1_000 },
                ..Default::default()
            })
            .collect();
        w.firm_history.insert(firm, history);
        for step in 0..60u32 {
            w.tick = cw_core::calendar::Tick(MIN_TENURE_WEEKS + 4 + step * 4);
            review(&mut w, &ds, firm);
        }
        assert_eq!(ceo(&w, firm), Some(old));
    }

    #[test]
    fn rotating_chairs_come_from_the_cast() {
        let ds = Dataset::load().unwrap();
        let mut w = seed::build(&ds, 21, ScenarioKind::Guided, ScenarioMode::Sandbox);
        assign(&mut w, &ds, "great-lakes-2000");
        let mut seen = std::collections::BTreeSet::new();
        for week in 1..=(crate::economy::politics::TERM_WEEKS * 5) {
            w.tick = cw_core::calendar::Tick(week);
            crate::economy::macro_cycle::weekly(&mut w, &ds);
            seen.insert(w.macro_state.cycle.chair.name.clone());
        }
        assert!(seen.len() >= 2, "chairs rotate: {seen:?}");
        for name in &seen {
            assert!(
                ds.cast.iter().any(|p| &p.name == name),
                "{name} is from the cast"
            );
        }
    }
}
