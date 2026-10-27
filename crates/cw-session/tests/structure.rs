//! Corporate structure through the session: group supply, transfer settings, bonds and
//! activist state survive a save and continue identically, every run of a seed is the same,
//! and the snapshot reports the new fields.

use cw_core::ids::Handle;
use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::state::{HolderId, ScenarioKind, ScenarioMode, ShareLot};

fn config(seed: u64) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    }
}

fn run(session: &mut Session, weeks: u32) {
    for _ in 0..weeks {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
    }
}

fn submit(session: &mut Session, intent: CommandIntent, id: u64) -> Result<(), String> {
    let envelope = CommandEnvelope {
        command_id_high: 0x57_0001,
        command_id_low: id,
        actor: INVESTOR_ACTOR,
        expected_revision: session.world.revision,
        submitted_tick: session.world.tick.0,
        intent,
    };
    session
        .submit(envelope)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

/// A session where the investor owns two firms outright so they form a group, with a bond out
/// and a transfer price off market.
fn structured(seed: u64) -> Session {
    let mut session = Session::create(config(seed)).unwrap();
    run(&mut session, 14);
    let primary = session.world.investor.primary_group.unwrap();
    let other: Handle = session
        .world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| {
            *h != primary
                && session.world.firm(*h).unwrap().operative()
                && session.world.firm(*h).unwrap().bank.is_none()
                && session.world.firm(*h).unwrap().insurer.is_none()
        })
        .unwrap();
    {
        let f = session.world.firms.get_mut(other).unwrap();
        let issued = f.shares.issued;
        f.shares.holders.clear();
        f.shares.holders.insert(
            HolderId::Investor,
            ShareLot {
                qty: issued * 8 / 10,
                cost_cents: 1,
            },
        );
        f.shares.holders.insert(
            HolderId::Outside(0),
            ShareLot {
                qty: issued - issued * 8 / 10,
                cost_cents: 1,
            },
        );
        f.board.ceo = Some(cw_sim::state::CeoRef::Investor);
    }
    session.world.outside.investors[0].capital_cents += 10_000_000_000;
    let divisions =
        cw_sim::economy::sectors::divisions_of(&session.world, &session.dataset, primary);
    if let Some(d) = divisions.first() {
        let _ = submit(
            &mut session,
            CommandIntent::SetTransferPrice {
                firm: primary,
                region: d.region,
                industry: d.industry,
                bps: 11_000,
            },
            1,
        );
    }
    let _ = submit(
        &mut session,
        CommandIntent::IssueBond {
            firm: primary,
            principal_cents: 50_000_000,
            coupon_bps: 0,
            weeks: 156,
        },
        2,
    );
    session
}

#[test]
fn structure_state_survives_a_save_and_continues_identically() {
    let mut session = structured(71);
    run(&mut session, 8);
    assert!(
        session.world.structure.stats.bonds_issued >= 1 || session.world.bonds.iter().count() > 0,
        "the bond was issued"
    );
    let bytes = session.export_save().unwrap();
    let mut restored = Session::import_save(&bytes).unwrap();
    assert_eq!(session.canonical_hash(), restored.canonical_hash());
    assert_eq!(
        session.world.structure.links.len(),
        restored.world.structure.links.len()
    );
    run(&mut session, 12);
    run(&mut restored, 12);
    assert_eq!(
        session.canonical_hash(),
        restored.canonical_hash(),
        "a loaded game and a continued one never diverge"
    );
}

#[test]
fn two_runs_of_one_seed_are_identical_with_structure_in_play() {
    let mut x = structured(72);
    let mut y = structured(72);
    run(&mut x, 30);
    run(&mut y, 30);
    assert_eq!(x.canonical_hash(), y.canonical_hash());
    assert_eq!(
        x.world.structure.stats.group_lots_moved,
        y.world.structure.stats.group_lots_moved
    );
}

#[test]
fn a_rejected_structure_command_leaves_the_world_unchanged() {
    let mut session = structured(73);
    let before = session.canonical_hash();
    let primary = session.world.investor.primary_group.unwrap();
    // A stake outside the allowed range, a bond far too small, and a bond order on nothing.
    assert!(
        submit(
            &mut session,
            CommandIntent::ListSubsidiary {
                firm: primary,
                stake_bps: 9_900
            },
            10
        )
        .is_err()
    );
    assert!(
        submit(
            &mut session,
            CommandIntent::IssueBond {
                firm: primary,
                principal_cents: 1_000,
                coupon_bps: 0,
                weeks: 156
            },
            11
        )
        .is_err()
    );
    assert!(
        submit(
            &mut session,
            CommandIntent::SellBond {
                bond: Handle(u64::MAX),
                qty: 100
            },
            12
        )
        .is_err()
    );
    assert!(
        submit(
            &mut session,
            CommandIntent::RespondActivist {
                campaign: 99,
                stance: 0
            },
            13
        )
        .is_err()
    );
    assert_eq!(session.canonical_hash(), before);
}

#[test]
fn the_snapshot_reports_structure() {
    let mut session = structured(74);
    run(&mut session, 11);
    let snap = session.market_snapshot();
    assert_eq!(snap.schema, cw_session::telemetry::SNAPSHOT_SCHEMA);
    assert!(snap.structure.groups >= 1);
    assert!(snap.structure.bonds_outstanding >= 1 || snap.structure.bonds_issued_total >= 1);
    assert!(snap.structure.avg_reputation_bps > 0);
    let json = serde_json::to_string(&snap).unwrap();
    assert!(json.contains("\"structure\""));
}
