//! Each scenario picks its own geography, lanes, firms and starting balance from content, and
//! Great Lakes 2000 plays exactly as it did before scenarios had their own data.

use cw_data::{Difficulty, PricePath};
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn config(seed: u64, kind: ScenarioKind, scenario: &str) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: kind,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario.to_owned(),
            path: Some(PricePath::HistoricalVariance),
        },
    }
}

fn run(seed: u64, kind: ScenarioKind, scenario: &str, weeks: u32) -> String {
    let mut session = Session::create(config(seed, kind, scenario)).unwrap();
    for _ in 0..weeks {
        let result = session.advance(1);
        if result.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
    }
    session.canonical_hash()
}

/// Great Lakes 2000 plays exactly as it did before scenarios had their own data. These were
/// recorded on the engine before the change and must never move because of Texas content.
#[test]
fn great_lakes_dataset_and_worlds_are_unchanged() {
    let data = cw_data::Dataset::load().unwrap();
    assert_eq!(data.scenario_id, "great-lakes-2000");
    assert_eq!(
        data.hash,
        "39b08bb5ef4e0785333729d93f3a3377034cf7dc3d8175428bfcc5c827b1b941"
    );
    let pins = [
        (
            1u64,
            "613ecc74dcca0b951390ca437ec6dd4344971436db677a7b4ac64e3427424589",
        ),
        (
            2,
            "91c20146de7fac1c1d7d6cf0f8c424664d7b2c012e8a17ca229b710fea4d74c4",
        ),
        (
            3,
            "37b20e43b2fa3bcd3fb1c4bcdf97376d72e206d9aba9bda5f828b72049a575bc",
        ),
    ];
    for (seed, hash) in pins {
        assert_eq!(
            run(seed, ScenarioKind::Guided, "great-lakes-2000", 52),
            hash,
            "seed {seed}"
        );
    }
    assert_eq!(
        run(4, ScenarioKind::Founder, "great-lakes-2000", 52),
        "d89e41cc47cba239df759efc53ee69dab873df6e6dc98ad5339dfcac30eea30c"
    );
    // No world prices and no scenario named: the engine fixture path.
    assert_eq!(
        run(5, ScenarioKind::Guided, "", 26),
        "a8af919afff0b6349c6af42f3e97b1c2632c12e2d53156665a18bcd81e2baed8"
    );
    // A scenario with no content of its own plays on the default dataset.
    let silicon = cw_data::Dataset::load_scenario("silicon-valley-1995").unwrap();
    assert_eq!(silicon.hash, data.hash);
}

#[test]
fn texas_has_its_own_geography_lanes_firms_and_balance() {
    let texas = cw_data::Dataset::load_scenario("texas-1981").unwrap();
    let lakes = cw_data::Dataset::load().unwrap();
    assert_ne!(texas.hash, lakes.hash);
    assert_eq!(texas.scenario_id, "texas-1981");
    assert_eq!(texas.states.len(), 3);
    assert_eq!(texas.regions.len(), 22);
    assert!(texas.seed_firms.len() >= 30);
    assert!(texas.validate().is_empty(), "{:?}", texas.validate());
    // Every pair of regions has a land lane.
    let n = texas.regions.len();
    assert_eq!(texas.lanes.len(), n * (n - 1) / 2);
    // The Gulf is a sea system joining the six ports, and Houston reaches New Orleans by sea.
    let houston = texas.region_index["TX-HOU"];
    let new_orleans = texas.region_index["LA-NOL"];
    assert!(texas.sea_km(houston, new_orleans).is_some());
    assert!(texas.public_ports[houston as usize]);
    assert_eq!(texas.public_ports.iter().filter(|p| **p).count(), 6);
    // Oil dearer and pay lower than the Great Lakes engine units.
    let price = |d: &cw_data::Dataset, id: &str| d.reference_price(d.product_index[id]);
    assert!(price(&texas, "crude_oil") > price(&lakes, "crude_oil"));
    assert!(texas.baseline_weekly_wage_cents[0] < lakes.baseline_weekly_wage_cents[0]);
    assert_eq!(price(&texas, "software"), price(&lakes, "software"));
    // The strategies and products are shared.
    assert_eq!(texas.recipes.len(), lakes.recipes.len());
    assert_eq!(texas.products.len(), lakes.products.len());
    assert_eq!(texas.player_region, "TX-PMB");
    assert_eq!(texas.player_recipe, "oil_wells");
}

#[test]
fn texas_pipelines_open_on_named_corridors_and_carry_crude() {
    let session = Session::create(config(1, ScenarioKind::Guided, "texas-1981")).unwrap();
    assert_eq!(session.dataset.scenario_id, "texas-1981");
    assert_eq!(session.world.regions.len(), 22);
    let pipeline = session.dataset.logistics.mode_index("pipeline").unwrap();
    let mut session = session;
    session.advance(2);
    let st = &session.world.logistics;
    let permian = session.dataset.region_index["TX-PMB"];
    let houston = session.dataset.region_index["TX-HOU"];
    let key = cw_sim::economy::logistics::LaneKey::new(permian, houston, pipeline);
    let lane = st
        .lanes
        .get(&key)
        .expect("the Permian to Houston pipeline exists");
    assert!(lane.cap() > 0);
}

#[test]
fn texas_runs_replays_and_saves_on_its_own_dataset() {
    let first = run(7, ScenarioKind::Guided, "texas-1981", 26);
    assert_eq!(first, run(7, ScenarioKind::Guided, "texas-1981", 26));
    assert_ne!(first, run(7, ScenarioKind::Guided, "great-lakes-2000", 26));

    let mut session = Session::create(config(8, ScenarioKind::Guided, "texas-1981")).unwrap();
    session.advance(10);
    let bytes = session.export_save().unwrap();
    let mut loaded = Session::import_save(&bytes).unwrap();
    assert_eq!(loaded.dataset.scenario_id, "texas-1981");
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
    session.advance(5);
    loaded.advance(5);
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());

    let log: cw_session::ReplayLog = serde_json::from_str(&session.export_replay()).unwrap();
    let mut fresh = Session::create(log.config.clone()).unwrap();
    fresh.advance(15);
    // The replay log's scenario reaches the same dataset and so the same world.
    assert_eq!(fresh.canonical_hash(), session.canonical_hash());
}

/// A short smoke inside the test suite; `examples/scenario_smoke.rs` runs the full 8 x 104.
#[test]
fn texas_opens_solvent_and_keeps_the_player_company_alive() {
    for seed in 1..=2u64 {
        let mut session =
            Session::create(config(seed, ScenarioKind::Guided, "texas-1981")).unwrap();
        for _ in 0..26 {
            let result = session.advance(1);
            if result.pause.is_some() {
                for event in session.world.events.pending.iter_mut() {
                    event.resolved = true;
                }
                session.dismiss_pause();
            }
        }
        let group = session.world.investor.primary_group.unwrap();
        assert!(
            session.world.firms.get(group).unwrap().operative(),
            "seed {seed}"
        );
        let dissolved = session
            .world
            .firms
            .iter()
            .filter(|(_, firm)| firm.archetype.is_some())
            .filter(|(_, firm)| matches!(firm.status, cw_sim::state::FirmStatus::Dissolved))
            .count();
        assert!(
            dissolved <= 2,
            "seed {seed}: {dissolved} named firms dissolved"
        );
    }
}
