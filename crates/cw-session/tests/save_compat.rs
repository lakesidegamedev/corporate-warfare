//! Saves written by released builds keep loading and playing.
//!
//! `fixtures/preview2-guided-week4.cwsave` was written by the released 1.8 preview 2 engine
//! (tag `v1.8.0-preview.2`): a guided career, seed 7, default scenario and difficulty, played four
//! weeks with every event resolved, then `Session::export_save`. The app's autosave and manual
//! save write the same bytes. Preview 1 had the same layout, so this covers both.

use cw_session::Session;
use cw_session::save::{self, SCHEMA_VERSION};

const PREVIEW2: &[u8] = include_bytes!("fixtures/preview2-guided-week4.cwsave");

/// The header fields of a save, read by hand from the documented envelope.
fn header(bytes: &[u8]) -> (u32, u32, u64) {
    let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let mut at = 12;
    for _ in 0..2 {
        let len = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
        at += 2 + len;
    }
    at += 8; // career
    let tick = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    at += 4 + 8; // tick, revision
    let declared = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
    (schema, tick, declared)
}

#[test]
fn a_preview_2_save_loads_with_neutral_defaults_for_the_new_fields() {
    let (schema, tick, _) = header(PREVIEW2);
    assert_eq!(schema, 1, "the fixture is an old-layout save");
    assert_eq!(tick, 4);
    let session = Session::import_save(PREVIEW2).expect("a preview 2 save must load");
    let world = &session.world;
    assert_eq!(world.tick.0, 4);
    assert_eq!(world.seed, 7);
    assert!(world.firms.iter().count() > 1, "player and rivals survive");
    assert!(world.facilities.iter().count() > 0);
    for (_, plant) in world.facilities.iter() {
        assert!(plant.cut_bps.is_empty(), "old plants keep the recipe split");
    }
    for (_, firm) in world.firms.iter() {
        assert!(!firm.policies.production_locked);
    }
    for region in &world.regions {
        assert!(region.households.wanted_milli.is_empty());
        assert!(region.households.unmet_milli.is_empty());
        assert!(
            !region.households.weights.is_empty(),
            "old household data kept"
        );
    }
}

#[test]
fn the_migrated_world_is_the_old_payload_plus_one_empty_byte_per_new_field() {
    // Every new field is last in its struct and defaults to an empty list, empty map or false,
    // each of which postcard writes as the single byte 0. So the current encoding of the
    // migrated world is exactly the old payload grown by one byte per new field per instance:
    // one per plant (cut), one per firm (lock) and two per region (wanted, unmet). This checks
    // the frozen layout consumed every old byte in the right frame without trusting the decoder.
    let (_, _, declared) = header(PREVIEW2);
    let (world, career) = save::decode(PREVIEW2).unwrap();
    let plants = world.facilities.iter().count();
    let firms = world.firms.iter().count();
    let regions = world.regions.len();
    let now = postcard::to_allocvec(&world).unwrap();
    assert_eq!(
        now.len() as u64,
        declared + plants as u64 + firms as u64 + 2 * regions as u64
    );
    // Written again, it is a current-layout save that round trips to the same world.
    let again = save::encode(&world, career).unwrap();
    assert_eq!(header(&again).0, SCHEMA_VERSION);
    let (restored, restored_career) = save::decode(&again).unwrap();
    assert_eq!(restored_career, career);
    assert_eq!(
        cw_sim::state::canonical_hash(&restored),
        cw_sim::state::canonical_hash(&world)
    );
}

#[test]
fn a_loaded_preview_2_career_plays_on_for_ten_weeks() {
    let mut session = Session::import_save(PREVIEW2).unwrap();
    for _ in 0..10 {
        let result = session.advance(1);
        if result.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
        for (_, firm) in session.world.firms.iter() {
            assert!(firm.book.is_balanced(), "{} ledger", firm.name);
        }
    }
    assert_eq!(session.world.tick.0, 14);
    // The new fields now carry real weeks, and the career saves and reloads in the new layout.
    assert!(
        session
            .world
            .regions
            .iter()
            .any(|r| !r.households.wanted_milli.is_empty())
    );
    let bytes = session.export_save().unwrap();
    assert_eq!(header(&bytes).0, SCHEMA_VERSION);
    let reloaded = Session::import_save(&bytes).unwrap();
    assert_eq!(reloaded.canonical_hash(), session.canonical_hash());
}

#[test]
fn an_old_layout_read_does_not_leak_into_later_reads() {
    let world = save::decode(PREVIEW2).unwrap().0;
    let current = save::encode(&world, 1).unwrap();
    // A failed old-layout read (the current bytes labelled schema 1) leaves the thread reading
    // the current layout, so the next current save still decodes.
    let mut mislabelled = current.clone();
    mislabelled[4..8].copy_from_slice(&1u32.to_le_bytes());
    let _ = save::decode(&mislabelled);
    assert!(save::decode(&current).is_ok());
}
