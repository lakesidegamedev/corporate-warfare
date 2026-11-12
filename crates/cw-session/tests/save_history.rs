//! Every earlier save opens in 2.0, plays a year and saves again.
//!
//! Each fixture was written by a real build, never by hand: the build of the named release or
//! merge commit was compiled in a temporary worktree and its `cw-cli run` wrote the save
//! (`cw-cli run --weeks 6 --seed 5 --sandbox --scenario-id <id>`, six weeks, no career), then
//! the worktree was deleted. `save_compat.rs` holds the older layouts (1 to 3) and the saves with
//! hand-added records; this file is the release history from 1.9.0 to the 2.0 previews.
//!
//! | label | build | save layout | what it lacks that 2.0 added |
//! |---|---|---|---|
//! | `v1.9.0` | tag `v1.9.0` | 3 | careers, raid campaigns, plant auctions, capacity rules |
//! | `v1.10-preview` | `v1.10.0-preview.2` engine | 3 | the same |
//! | `2.0 <sha>` | merge commits of the 2.0 branch, in order: `d82a1f3` (rivals that build), `637312c` (raid campaigns, layout 4), `4e25ea5` (demand), `8ea05c4` (auctions and mergers), `5299caa` (world prices), `123ac61` (careers) | 3 or 4 | whatever landed after them |
//! | `2.0 preview.1` | tag `v2.0.0-preview.1` (engine of `d14c361`) | 4 | the later balance changes |
//!
//! Every fixture is checked four ways: it opens at the layout and week it was written at; no
//! career is forced on it (a save with no career plays on as a sandbox); it plays 52 weeks with
//! balanced books and nothing unanswered left to stall it; and the save it then writes is the
//! current layout and reloads to the same world.

use cw_session::Session;
use cw_session::save::SCHEMA_VERSION;
use cw_sim::state::ScenarioMode;

struct Fixture {
    label: &'static str,
    bytes: &'static [u8],
    layout: u32,
    scenario: &'static str,
}

macro_rules! fixture {
    ($label:literal, $file:literal, $layout:literal, $scenario:literal) => {
        Fixture {
            label: $label,
            bytes: include_bytes!(concat!("fixtures/", $file)),
            layout: $layout,
            scenario: $scenario,
        }
    };
}

const FIXTURES: &[Fixture] = &[
    fixture!(
        "v1.9.0",
        "v1.9.0-great-lakes-2000-sandbox-week6.cwsave",
        3,
        "great-lakes-2000"
    ),
    fixture!(
        "v1.9.0",
        "v1.9.0-texas-1981-sandbox-week6.cwsave",
        3,
        "texas-1981"
    ),
    fixture!(
        "v1.9.0",
        "v1.9.0-silicon-valley-1995-sandbox-week6.cwsave",
        3,
        "silicon-valley-1995"
    ),
    fixture!(
        "v1.9.0",
        "v1.9.0-ruhr-rhine-1991-sandbox-week6.cwsave",
        3,
        "ruhr-rhine-1991"
    ),
    fixture!(
        "v1.10-preview2",
        "v1.10-preview2-silicon-valley-1995-sandbox-week6.cwsave",
        3,
        "silicon-valley-1995"
    ),
    fixture!(
        "2.0 d82a1f3",
        "2.0-d82a1f3-texas-1981-sandbox-week6.cwsave",
        3,
        "texas-1981"
    ),
    fixture!(
        "2.0 637312c",
        "2.0-637312c-ruhr-rhine-1991-sandbox-week6.cwsave",
        4,
        "ruhr-rhine-1991"
    ),
    fixture!(
        "2.0 4e25ea5",
        "2.0-4e25ea5-great-lakes-2000-sandbox-week6.cwsave",
        4,
        "great-lakes-2000"
    ),
    fixture!(
        "2.0 8ea05c4",
        "2.0-8ea05c4-texas-1981-sandbox-week6.cwsave",
        4,
        "texas-1981"
    ),
    fixture!(
        "2.0 5299caa",
        "2.0-5299caa-ruhr-rhine-1991-sandbox-week6.cwsave",
        4,
        "ruhr-rhine-1991"
    ),
    fixture!(
        "2.0 123ac61",
        "2.0-123ac61-silicon-valley-1995-sandbox-week6.cwsave",
        4,
        "silicon-valley-1995"
    ),
    fixture!(
        "2.0 preview.1",
        "2.0-preview1-great-lakes-2000-sandbox-week6.cwsave",
        4,
        "great-lakes-2000"
    ),
    fixture!(
        "2.0 preview.1",
        "2.0-preview1-texas-1981-sandbox-week6.cwsave",
        4,
        "texas-1981"
    ),
];

/// The header fields of a save, read by hand from the documented envelope: schema and week.
fn header(bytes: &[u8]) -> (u32, u32) {
    let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let mut at = 12;
    for _ in 0..2 {
        let len = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
        at += 2 + len;
    }
    at += 8; // career
    let tick = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    (schema, tick)
}

/// Play `weeks` weeks the way a patient player does: answer every card, dismiss every pause.
fn play(session: &mut Session, weeks: u32, what: &str) {
    for _ in 0..weeks {
        let result = session.advance(1);
        if result.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
        for (_, firm) in session.world.firms.iter() {
            assert!(
                firm.book.is_balanced(),
                "{what}: {} ledger unbalanced in week {}",
                firm.name,
                session.world.tick.0
            );
        }
    }
}

#[test]
fn every_earlier_save_opens_at_the_week_and_layout_it_was_written() {
    for fixture in FIXTURES {
        let what = format!("{} {}", fixture.label, fixture.scenario);
        let (schema, tick) = header(fixture.bytes);
        assert_eq!(schema, fixture.layout, "{what}: layout");
        assert_eq!(tick, 6, "{what}: week");
        let session = Session::import_save(fixture.bytes)
            .unwrap_or_else(|error| panic!("{what} must load: {error:?}"));
        assert_eq!(session.world.tick.0, 6, "{what}");
        assert_eq!(session.world.seed, 5, "{what}");
        assert_eq!(session.dataset.scenario_id, fixture.scenario, "{what}");
        assert!(
            session.world.firms.iter().count() > 1,
            "{what}: rivals survive"
        );
        assert!(
            session.world.facilities.iter().count() > 0,
            "{what}: plants survive"
        );
        if fixture.layout < 4 {
            assert!(
                session.world.raids.campaigns.is_empty(),
                "{what}: no raid campaign is invented"
            );
        }
    }
}

#[test]
fn an_old_save_without_a_career_plays_on_as_a_sandbox() {
    for fixture in FIXTURES {
        let what = format!("{} {}", fixture.label, fixture.scenario);
        let mut session = Session::import_save(fixture.bytes).unwrap();
        assert!(session.world.career.is_none(), "{what}: no career on load");
        assert_eq!(
            session.world.mode,
            ScenarioMode::Sandbox,
            "{what}: sandbox mode"
        );
        assert!(
            session.career_standing().is_none(),
            "{what}: nothing to stand against"
        );
        play(&mut session, 52, &what);
        // A year on, still no career, no ending, no board warning forced on a game that never
        // asked for one, and no five-year score sheet.
        assert!(
            session.world.career.is_none(),
            "{what}: no career after a year"
        );
        assert!(session.career_summary().is_none(), "{what}: no ending");
        assert!(session.world.score.is_none(), "{what}: no score sheet");
    }
}

#[test]
fn every_earlier_save_plays_a_year_and_saves_again_as_the_current_layout() {
    for fixture in FIXTURES {
        let what = format!("{} {}", fixture.label, fixture.scenario);
        let mut session = Session::import_save(fixture.bytes).unwrap();
        play(&mut session, 52, &what);
        assert_eq!(session.world.tick.0, 58, "{what}");
        let bytes = session.export_save().unwrap();
        assert_eq!(
            header(&bytes),
            (SCHEMA_VERSION, 58),
            "{what}: re-saved header"
        );
        let mut reloaded = Session::import_save(&bytes).unwrap();
        assert_eq!(
            reloaded.canonical_hash(),
            session.canonical_hash(),
            "{what}: the re-save reloads to the same world"
        );
        // And the reloaded game is still alive: it plays on and matches the original's next weeks.
        play(&mut session, 4, &what);
        play(&mut reloaded, 4, &what);
        assert_eq!(
            reloaded.canonical_hash(),
            session.canonical_hash(),
            "{what}: the reloaded game plays the same next weeks"
        );
    }
}
