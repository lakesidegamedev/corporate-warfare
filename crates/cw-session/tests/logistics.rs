//! Freight through the session: lane state, contracts and policies survive a save and continue
//! identically, alerts fire only when a lane matters to the player, and telemetry reports use,
//! rates and the outside carriers' share.

use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::economy::logistics::{LaneKey, ShipKind, ShipPolicy};
use cw_sim::state::{ScenarioKind, ScenarioMode};

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

fn submit(session: &mut Session, intent: CommandIntent, id: u64) {
    let envelope = CommandEnvelope {
        command_id_high: 0x10_6157,
        command_id_low: id,
        actor: INVESTOR_ACTOR,
        expected_revision: session.world.revision,
        submitted_tick: session.world.tick.0,
        intent,
    };
    session.submit(envelope).expect("command accepted");
}

#[test]
fn lanes_contracts_and_policies_survive_a_save_and_continue_identically() {
    let mut session = Session::create(config(61)).unwrap();
    let firm = session.world.investor.primary_group.unwrap();
    run(&mut session, 12);
    // A policy on the player's own division and a lane contract held by a rival group.
    let plant = session.world.firm(firm).unwrap().facilities[0];
    submit(
        &mut session,
        CommandIntent::SetShippingPolicy {
            facility: plant,
            kind: 1,
            mode: Some(1),
            carrier: None,
        },
        1,
    );
    assert_eq!(
        session.world.facilities.get(plant).unwrap().ship,
        ShipPolicy {
            kind: ShipKind::Fastest,
            mode: Some(1),
            carrier: None
        }
    );
    let (a, b) = {
        let hot = cw_sim::economy::logistics::hot_lanes(&session.world, &session.dataset);
        let key = hot.first().map(|h| h.key).unwrap_or(LaneKey::new(0, 1, 0));
        (key.a, key.b)
    };
    let rival = session
        .world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| {
            !cw_ai::player_controlled(&session.world, *h)
                && session.world.firm(*h).unwrap().operative()
        })
        .unwrap();
    session
        .world
        .logistics
        .contracts
        .push(cw_sim::economy::logistics::LaneContract {
            id: 900,
            holder: rival,
            key: LaneKey::new(a, b, 0),
            lots: 10,
            held: 0,
            used: 0,
            fee_per_lot_cents: 100,
            start: session.world.tick.0,
            weeks_left: 20,
        });
    let bytes = session.export_save().unwrap();
    let mut restored = Session::import_save(&bytes).unwrap();
    assert_eq!(session.canonical_hash(), restored.canonical_hash());
    assert!(!restored.world.logistics.carriers.is_empty());
    assert_eq!(restored.world.logistics.contracts.len(), 1);
    run(&mut session, 10);
    run(&mut restored, 10);
    assert_eq!(
        session.canonical_hash(),
        restored.canonical_hash(),
        "a loaded game and a continued one never diverge"
    );
}

#[test]
fn two_runs_of_one_seed_are_identical() {
    let mut x = Session::create(config(62)).unwrap();
    let mut y = Session::create(config(62)).unwrap();
    run(&mut x, 25);
    run(&mut y, 25);
    assert_eq!(x.canonical_hash(), y.canonical_hash());
}

#[test]
fn telemetry_reports_lanes_rates_and_the_outside_share() {
    let mut session = Session::create(config(63)).unwrap();
    run(&mut session, 20);
    let snapshot = session.market_snapshot();
    assert_eq!(snapshot.schema, 3);
    let l = &snapshot.logistics;
    assert_eq!(l.mode_ids, ["truck", "rail", "pipeline", "sea"]);
    assert_eq!(l.lots_by_mode.len(), 4);
    assert!(l.lots_by_mode.iter().sum::<i64>() > 0, "freight moved");
    assert!(l.freight_paid_cents > 0 && l.shipments > 0);
    assert!(l.outside_share_bps >= 0 && l.outside_share_bps <= 10_000);
    assert!(l.avg_milli_cents_per_lot_km > 0);
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(json.contains("outside_share_bps") && json.contains("congested_lanes"));
    let player = snapshot.player.expect("a player company");
    assert!(player.group_lots_shipped >= 0);
}

#[test]
fn freight_alerts_fire_only_for_lanes_the_player_ships_on() {
    let mut session = Session::create(config(64)).unwrap();
    run(&mut session, 6);
    let firm = session.world.investor.primary_group.unwrap();
    let key = LaneKey::new(0, 1, 0);
    let tick = session.world.tick.0;
    // A congested lane the player does not use is not news.
    session.world.logistics.stats.insert(
        key,
        cw_sim::economy::logistics::LaneStat {
            hot_weeks: 6,
            ..Default::default()
        },
    );
    let keys = |s: &Session| {
        s.overview()
            .alerts
            .into_iter()
            .map(|a| a.key)
            .collect::<Vec<_>>()
    };
    assert!(!keys(&session).contains(&"alert.lane_congested".to_owned()));
    // Once the player ships on it, four weeks of congestion is.
    session
        .world
        .logistics
        .stats
        .get_mut(&key)
        .unwrap()
        .users
        .push((firm, tick));
    assert!(keys(&session).contains(&"alert.lane_congested".to_owned()));
    // A rival reserving that lane is news too, for three weeks.
    let rival = session
        .world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| !cw_ai::player_controlled(&session.world, *h))
        .unwrap();
    session
        .world
        .logistics
        .reservations
        .push(cw_sim::economy::logistics::Reservation {
            tick,
            key,
            holder: rival,
            lots: 50,
        });
    assert!(keys(&session).contains(&"alert.lane_reserved".to_owned()));
    session.world.logistics.reservations.clear();
    // A rate hike of a fifth over four weeks.
    let stat = session.world.logistics.stats.get_mut(&key).unwrap();
    stat.hot_weeks = 0;
    stat.mult_hist = vec![8_000, 8_000, 8_500, 9_000, 10_000];
    assert!(keys(&session).contains(&"alert.carrier_hike".to_owned()));
    session
        .world
        .logistics
        .stats
        .get_mut(&key)
        .unwrap()
        .mult_hist = vec![8_000, 8_000, 8_000, 8_100, 8_200];
    assert!(
        keys(&session)
            .iter()
            .all(|k| !k.starts_with("alert.lane") && k != "alert.carrier_hike")
    );
}
