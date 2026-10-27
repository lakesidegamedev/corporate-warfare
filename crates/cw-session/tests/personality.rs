//! Rival personalities in play: with a chief executive of each temperament forced onto a share
//! of the rivals, the moves they make differ the way the temperaments say, the whole run is
//! deterministic, a save taken mid-run continues identically, and grudges and alliances are
//! recorded and fade.

use cw_data::{Difficulty, PricePath, Role, Temperament};
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn session(seed: u64) -> Session {
    session_with(seed, |i| Temperament::ALL[i % 6])
}

fn session_with(seed: u64, pick: impl Fn(usize) -> Temperament) -> Session {
    let mut s = Session::create(ScenarioConfig {
        seed,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: "great-lakes-2000".to_owned(),
            path: Some(PricePath::HistoricalVariance),
        },
    })
    .unwrap();
    // Every rival gets a chief executive, cycling through the six temperaments.
    let person = |t: Temperament| {
        s.dataset
            .cast
            .iter()
            .position(|p| p.temperament == t && p.can(Role::Ceo))
            .unwrap() as u8
    };
    let people: Vec<u8> = (0..64).map(|i| person(pick(i))).collect();
    let rivals: Vec<_> = s
        .world
        .firms
        .handles_sorted()
        .into_iter()
        .filter(|h| s.world.firms.get(*h).is_some_and(|f| f.archetype.is_some()))
        .collect();
    for (i, h) in rivals.iter().enumerate() {
        s.world.firms.get_mut(*h).unwrap().ceo = Some(people[i]);
    }
    s
}

fn play(s: &mut Session, weeks: u32) {
    for _ in 0..weeks {
        let r = s.advance(1);
        if r.pause.is_some() {
            for e in s.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            s.dismiss_pause();
        }
    }
}

fn pair(s: &Session, key: &str) -> u32 {
    s.world
        .rivalry
        .stats
        .moves_by_pair
        .get(key)
        .copied()
        .unwrap_or(0)
}

/// Every rival run by one temperament, same seeds: the moves of one class they make in 78 weeks.
fn class_count(t: Temperament, class: &str) -> u32 {
    (1..=3u64)
        .map(|seed| {
            let mut s = session_with(seed, |_| t);
            play(&mut s, 78);
            pair(&s, &format!("{}/{class}", t.key()))
        })
        .sum()
}

#[test]
fn each_temperament_plays_its_own_game() {
    use Temperament::*;
    let raiders_raid = class_count(Raider, "raid");
    let cautious_raid = class_count(Cautious, "raid");
    assert!(
        raiders_raid > cautious_raid * 2 && raiders_raid >= 6,
        "raiders raid far more than the cautious: {raiders_raid} vs {cautious_raid}"
    );
    let builders_build = class_count(Builder, "build");
    let cautious_build = class_count(Cautious, "build");
    assert!(
        builders_build > cautious_build,
        "builders build more than the cautious: {builders_build} vs {cautious_build}"
    );
    let speculators = class_count(Speculator, "speculate");
    let builders_spec = class_count(Builder, "speculate");
    assert!(
        speculators > builders_spec,
        "speculators stockpile more than builders: {speculators} vs {builders_spec}"
    );
    let financiers = class_count(Financier, "takeover");
    let builders_bids = class_count(Builder, "takeover");
    assert!(
        financiers > builders_bids,
        "financiers bid more than builders: {financiers} vs {builders_bids}"
    );
}

#[test]
fn the_run_is_deterministic_and_a_mid_run_save_continues_identically() {
    let mut a = session(4);
    let mut b = session(4);
    play(&mut a, 60);
    play(&mut b, 30);
    let bytes = b.export_save().unwrap();
    let mut c = Session::import_save(&bytes).unwrap();
    play(&mut b, 30);
    play(&mut c, 30);
    assert_eq!(a.canonical_hash(), b.canonical_hash());
    assert_eq!(b.canonical_hash(), c.canonical_hash());
    assert_eq!(a.world.rivalry, c.world.rivalry, "memory survives a save");
}

#[test]
fn raids_create_grudges_and_time_fades_them() {
    let mut s = session(2);
    play(&mut s, 104);
    let formed = s.world.rivalry.stats.grudges_formed;
    assert!(formed > 0, "104 weeks of rivalry leave grudges");
    // Stop all new offences by removing every raid chance: just let time pass with no rivals
    // acting is not possible, so check the decay rule on the live table instead.
    let strongest = s
        .world
        .rivalry
        .grudges
        .values()
        .map(|m| m.score)
        .max()
        .unwrap_or(0);
    let mut table = s.world.clone();
    for _ in 0..300 {
        cw_sim::economy::rivalry::weekly(&mut table);
    }
    let after = table
        .rivalry
        .grudges
        .values()
        .map(|m| m.score)
        .max()
        .unwrap_or(0);
    assert!(
        after < strongest.max(1) / 2 || strongest == 0,
        "{strongest} -> {after}"
    );
}

#[test]
fn the_dossier_lines_name_grudges_and_friends() {
    let mut s = session(3);
    play(&mut s, 104);
    let mut with_line = 0;
    for (handle, f) in s.world.firms.iter() {
        if f.archetype.is_none() {
            continue;
        }
        let g = cw_sim::economy::rivalry::grudges_of(&s.world, handle);
        let a = cw_sim::economy::rivalry::allies_of(&s.world, handle);
        with_line += (!g.is_empty() || !a.is_empty()) as u32;
    }
    assert!(
        with_line > 0,
        "some rival holds a grudge or a friend after two years"
    );
}
