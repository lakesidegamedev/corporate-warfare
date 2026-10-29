//! Scenario timelines: each effect fires on its date and only then, Off fires nothing, Shuffled is
//! deterministic per seed, and a saved and reloaded career plays what an unbroken one does.
//!
//! The effect tests run two careers from one seed side by side, one with a single event injected
//! at a known week and a twin without it, so the expected change is read off the twin and not
//! computed by the code under test.

use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::economy::systemic;
use cw_sim::economy::timeline::{Effect, LiveEvent, TimelineMode};

fn config(scenario: &str, seed: u64, timeline: Option<TimelineMode>) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: cw_sim::state::ScenarioKind::Guided,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario.to_owned(),
            path: None,
            timeline,
        },
    }
}

/// One week, answering any card that stops the clock the same way every time.
fn week(session: &mut Session) {
    let result = session.advance(1);
    if result.pause.is_some() {
        for event in session.world.events.pending.iter_mut() {
            event.resolved = true;
        }
        session.dismiss_pause();
    }
}

fn play(session: &mut Session, until: u32) {
    while session.world.tick.0 < until {
        week(session);
    }
}

const FIRES: u32 = 10;

fn inject(session: &mut Session, effects: Vec<Effect>) {
    let timeline = &mut session.world.systemic.timeline;
    timeline.mode = TimelineMode::Historical;
    timeline.events.push(LiveEvent {
        id: "probe".to_owned(),
        title: "Probe".to_owned(),
        summary: "A test event.".to_owned(),
        tick: FIRES,
        foreshadow_weeks: 3,
        effects,
        announced: false,
        fired_at: 0,
    });
}

/// Run an injected career and its twin to week `FIRES + after`, calling `probe(ours, twin)` at
/// the end of every week from 1.
fn pair(effects: Vec<Effect>, after: u32, mut probe: impl FnMut(u32, &Session, &Session)) {
    let mut ours = Session::create(config("texas-1981", 5, Some(TimelineMode::Off))).unwrap();
    let mut twin = Session::create(config("texas-1981", 5, Some(TimelineMode::Off))).unwrap();
    inject(&mut ours, effects);
    for t in 1..=FIRES + after {
        week(&mut ours);
        week(&mut twin);
        assert_eq!(ours.world.tick.0, t);
        probe(t, &ours, &twin);
    }
}

fn product(session: &Session, id: &str) -> u16 {
    session.dataset.product_index[id]
}

#[test]
fn a_price_shock_moves_the_family_index_on_its_date_and_fades() {
    let fuel = {
        let s = Session::create(config("texas-1981", 5, Some(TimelineMode::Off))).unwrap();
        product(&s, "fuel")
    };
    let effect = Effect::PriceShock {
        good: fuel,
        percent: -30,
        weeks: 20,
    };
    pair(vec![effect], 24, |t, ours, twin| {
        let family = ours.world.world_prices.product_family[&fuel] as usize;
        let (a, b) = (
            &ours.world.world_prices.families[family],
            &twin.world.world_prices.families[family],
        );
        if t < FIRES {
            assert_eq!(
                a.index_bps, b.index_bps,
                "week {t}: nothing before the date"
            );
        } else if t == FIRES {
            // The twin's index is raw * (1 + tilt); ours is raw * (1 + tilt - 30 percent).
            let expected = b.raw_bps as i64 * 3_000 / 10_000;
            let gap = (b.index_bps - a.index_bps) as i64;
            assert!((gap - expected).abs() <= 1, "gap {gap} expected {expected}");
        } else if t < FIRES + 20 {
            assert!(a.index_bps < b.index_bps, "week {t}: still below the twin");
        } else if t == FIRES + 20 {
            // The overlay has faded to nothing in the week the window closes.
            assert_eq!(
                cw_sim::economy::timeline::price_shock_bps(&ours.world, family as u16),
                0
            );
        }
    });
}

#[test]
fn a_demand_regime_applies_for_its_weeks_only() {
    let effect = Effect::DemandRegime {
        sector: "all".to_owned(),
        percent: -8,
        weeks: 6,
    };
    pair(vec![effect], 10, |t, ours, twin| {
        let tagged = |s: &Session| {
            s.world.regions[0]
                .demand_modifiers
                .iter()
                .filter(|(cause, ..)| cause == "timeline:probe")
                .count()
        };
        let ratio_ours = ours.world.demand_ratio_bps(0, None);
        let ratio_twin = twin.world.demand_ratio_bps(0, None);
        if (FIRES..FIRES + 6).contains(&t) {
            assert_eq!(tagged(ours), 1, "week {t}");
            assert!(
                ratio_ours < ratio_twin,
                "week {t}: {ratio_ours} {ratio_twin}"
            );
        } else {
            assert_eq!(tagged(ours), 0, "week {t}: outside the window");
        }
        assert_eq!(tagged(twin), 0);
        if t == FIRES {
            // Oracle: the twin's ratio times 92 percent, to rounding.
            let expected = ratio_twin * 9_200 / 10_000;
            assert!(
                (ratio_ours - expected).abs() <= 2,
                "{ratio_ours} vs {expected}"
            );
        }
    });
}

#[test]
fn a_sector_demand_regime_touches_only_that_sectors_goods() {
    let effect = Effect::DemandRegime {
        sector: "energy".to_owned(),
        percent: 6,
        weeks: 8,
    };
    pair(vec![effect], 2, |t, ours, _| {
        let electricity = product(ours, "electricity");
        let steel = product(ours, "steel");
        let (e, s) = (
            ours.world.regions[0]
                .demand_modifiers
                .iter()
                .any(|(cause, subject, ..)| {
                    cause == "timeline:probe" && *subject == Some(electricity)
                }),
            ours.world.regions[0]
                .demand_modifiers
                .iter()
                .any(|(cause, subject, ..)| cause == "timeline:probe" && *subject == Some(steel)),
        );
        assert_eq!((e, s), (t >= FIRES, false), "week {t}");
    });
}

#[test]
fn a_stance_shift_moves_the_chair_once() {
    pair(
        vec![Effect::CentralBankStance { shift: -25 }],
        5,
        |t, ours, twin| {
            let (a, b) = (
                ours.world.macro_state.cycle.chair.stance,
                twin.world.macro_state.cycle.chair.stance,
            );
            assert_eq!(a - b, if t >= FIRES { -25 } else { 0 }, "week {t}");
        },
    );
}

#[test]
fn a_credit_tightening_adds_to_the_lending_premium_for_its_weeks() {
    let effect = Effect::CreditTightening {
        spread_bps: 120,
        weeks: 8,
    };
    pair(vec![effect], 12, |t, ours, twin| {
        let spread = cw_sim::economy::timeline::credit_spread_bps(&ours.world);
        let expect = if (FIRES..FIRES + 8).contains(&t) {
            120
        } else {
            0
        };
        assert_eq!(spread, expect, "week {t}");
        assert_eq!(cw_sim::economy::timeline::credit_spread_bps(&twin.world), 0);
        if t == FIRES {
            let premium = |s: &Session| cw_sim::economy::macro_cycle::lending_premium_bps(&s.world);
            assert_eq!(premium(ours) - premium(twin), 120);
        }
    });
}

#[test]
fn a_law_change_sets_the_law_on_its_date() {
    let effects = vec![
        Effect::LawChange {
            law: "privatise".to_owned(),
            value: 1,
        },
        Effect::LawChange {
            law: "corporate_tax_bps".to_owned(),
            value: 3_100,
        },
        Effect::LawChange {
            law: "unions".to_owned(),
            value: 2,
        },
    ];
    pair(effects, 3, |t, ours, twin| {
        let laws = &ours.world.politics.laws;
        if t < FIRES {
            assert_eq!(*laws, twin.world.politics.laws, "week {t}");
        } else {
            assert!(laws.privatise);
            assert_eq!(laws.corporate_tax_bps, 3_100);
            assert_eq!(laws.unions, cw_sim::state::UnionLaw::Strong);
        }
    });
}

#[test]
fn a_systemic_trigger_strikes_on_its_date_and_is_recorded() {
    let effect = Effect::SystemicEvent {
        event: "harvest_failure".to_owned(),
    };
    pair(vec![effect], 2, |t, ours, _| {
        let harvests = ours
            .world
            .systemic
            .incidents
            .iter()
            .filter(|i| i.kind == systemic::HARVEST_FAILURE && i.week == FIRES)
            .count();
        let timeline_rows = ours
            .world
            .systemic
            .incidents
            .iter()
            .filter(|i| i.kind == systemic::TIMELINE)
            .count();
        assert_eq!(
            (harvests > 0, timeline_rows),
            (t >= FIRES, usize::from(t >= FIRES)),
            "week {t}"
        );
    });
}

#[test]
fn an_import_ceiling_change_scales_the_goods_gateways_once() {
    let steel = {
        let s = Session::create(config("texas-1981", 5, Some(TimelineMode::Off))).unwrap();
        product(&s, "steel")
    };
    pair(
        vec![Effect::ImportCeiling {
            good: steel,
            percent: -40,
        }],
        4,
        |t, ours, twin| {
            let caps = |s: &Session| -> Vec<i64> {
                s.world
                    .gateways
                    .iter()
                    .filter(|g| g.product == steel)
                    .map(|g| g.capacity_lots)
                    .collect()
            };
            let (a, b) = (caps(ours), caps(twin));
            assert!(!a.is_empty(), "steel has gateways");
            if t < FIRES {
                assert_eq!(a, b, "week {t}");
            } else if t == FIRES {
                let expected: Vec<i64> = b.iter().map(|c| (c * 60 / 100).max(1)).collect();
                assert_eq!(a, expected);
            }
        },
    );
}

#[test]
fn the_notice_goes_out_ahead_and_never_earlier() {
    let mut ours = Session::create(config("texas-1981", 5, Some(TimelineMode::Off))).unwrap();
    inject(&mut ours, vec![Effect::CentralBankStance { shift: 5 }]);
    for t in 1..=FIRES + 1 {
        week(&mut ours);
        let listed = cw_sim::economy::timeline::upcoming(&ours.world);
        let noticed = ours
            .world
            .politics
            .news
            .iter()
            .any(|n| n.text.starts_with("Coming"));
        // Foreshadow is 3 weeks: announced from week 7, gone from the list once it fires.
        let expect = (FIRES - 3..FIRES).contains(&t);
        assert_eq!(listed.len(), usize::from(expect), "week {t}");
        assert_eq!(noticed, t >= FIRES - 3, "week {t}");
    }
    let past = cw_sim::economy::timeline::past(&ours.world);
    assert_eq!(past.len(), 1);
    assert_eq!(past[0].tick, FIRES);
}

#[test]
fn off_fires_nothing_and_historical_plays_texas_on_its_dates() {
    let mut off = Session::create(config("texas-1981", 3, Some(TimelineMode::Off))).unwrap();
    assert!(off.world.systemic.timeline.events.is_empty());
    play(&mut off, 120);
    assert!(
        off.world
            .systemic
            .incidents
            .iter()
            .all(|i| i.kind != systemic::TIMELINE)
    );
    assert!(cw_sim::economy::timeline::past(&off.world).is_empty());

    let mut on = Session::create(config("texas-1981", 3, None)).unwrap();
    assert_eq!(on.world.systemic.timeline.mode, TimelineMode::Historical);
    // The regional bank failure is 1982 week 28, which is week 52 + 28 = 80, noticed 6 weeks ahead.
    play(&mut on, 73);
    assert!(cw_sim::economy::timeline::upcoming(&on.world).is_empty());
    play(&mut on, 74);
    let ahead = cw_sim::economy::timeline::upcoming(&on.world);
    assert_eq!(ahead.len(), 1);
    assert_eq!(
        (ahead[0].id.as_str(), ahead[0].tick),
        ("tx-regional-bank-failure", 80)
    );
    assert_eq!((ahead[0].year, ahead[0].week_of_year), (1982, 28));
    play(&mut on, 79);
    // The 1981 tight-money squeeze (60 bps for 39 weeks from week 30) is over by week 69.
    assert_eq!(cw_sim::economy::timeline::credit_spread_bps(&on.world), 0);
    play(&mut on, 80);
    assert_eq!(cw_sim::economy::timeline::credit_spread_bps(&on.world), 200);
    let past = cw_sim::economy::timeline::past(&on.world);
    assert_eq!(past[0].id, "tx-regional-bank-failure");
    assert_eq!(past[0].tick, 80);
}

#[test]
fn shuffled_is_deterministic_per_seed_and_stays_within_26_weeks() {
    let dates = |seed: u64| -> Vec<(String, u32)> {
        let s = Session::create(config(
            "silicon-valley-1995",
            seed,
            Some(TimelineMode::Shuffled),
        ))
        .unwrap();
        s.world
            .systemic
            .timeline
            .events
            .iter()
            .map(|e| (e.id.clone(), e.tick))
            .collect()
    };
    let historical: std::collections::BTreeMap<String, u32> = {
        let s = Session::create(config(
            "silicon-valley-1995",
            1,
            Some(TimelineMode::Historical),
        ))
        .unwrap();
        s.world
            .systemic
            .timeline
            .events
            .iter()
            .map(|e| (e.id.clone(), e.tick))
            .collect()
    };
    let (a, b, c) = (dates(21), dates(21), dates(22));
    assert_eq!(a, b, "same seed, same dates");
    assert_ne!(a, c, "a different seed moves them differently");
    assert_eq!(a.len(), historical.len(), "same events");
    let mut moved = 0;
    for (id, tick) in &a {
        let base = historical[id] as i64;
        assert!((*tick as i64 - base).abs() <= 26, "{id}: {tick} vs {base}");
        assert!(*tick >= 2);
        moved += usize::from(*tick as i64 != base);
    }
    assert!(moved >= a.len() / 2, "most events move: {moved}");
}

#[test]
fn a_saved_career_mid_timeline_plays_on_exactly_as_an_unbroken_one() {
    for stop in [76, 84] {
        // 76 is between the notice (74) and the event (80); 84 is after it fired.
        let mut unbroken = Session::create(config("texas-1981", 9, None)).unwrap();
        let mut saved = Session::create(config("texas-1981", 9, None)).unwrap();
        play(&mut unbroken, stop);
        play(&mut saved, stop);
        let bytes = saved.export_save().unwrap();
        let mut reloaded = Session::import_save(&bytes).unwrap();
        assert_eq!(
            reloaded.canonical_hash(),
            unbroken.canonical_hash(),
            "at {stop}"
        );
        play(&mut unbroken, 140);
        play(&mut reloaded, 140);
        assert_eq!(
            reloaded.canonical_hash(),
            unbroken.canonical_hash(),
            "reloaded at {stop} drifted from the unbroken run"
        );
        assert_eq!(
            reloaded.world.systemic.timeline,
            unbroken.world.systemic.timeline
        );
    }
}
