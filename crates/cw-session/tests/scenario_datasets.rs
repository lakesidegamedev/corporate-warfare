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
/// Repinned when the character cast joined the dataset and the engine began naming chairs and
/// chief executives from it.
#[test]
fn great_lakes_dataset_and_worlds_are_unchanged() {
    let data = cw_data::Dataset::load().unwrap();
    assert_eq!(data.scenario_id, "great-lakes-2000");
    assert_eq!(
        data.hash,
        "519ebc7d1c7319f75d000d688822a9b707c90f06218f9e6ebd5af793713edcd7"
    );
    let pins = [
        (
            1u64,
            "fba0787b20ddeb2510d04062155f162dfaaa1fd3410a896ac8e25e20b792d794",
        ),
        (
            2,
            "8d7c91ae4dc1b250642d365c5084698b8e8350a2d585da94e1a10f0c96a2da48",
        ),
        (
            3,
            "1f267d06d108dc4641cb44579c06ef5d05f86f540108d8892b449cd1f1665abc",
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
        "faddda8adffdef6bbff358d0a0394d7878200ce7f310150015a697024c17c4b2"
    );
    // No world prices and no scenario named: the engine fixture path.
    assert_eq!(
        run(5, ScenarioKind::Guided, "", 26),
        "ac180b862d07794f14dfff634b295c969b009bd963c49a2abb558c3388be07ea"
    );
    // A scenario with no content of its own plays on the default dataset.
    let ruhr = cw_data::Dataset::load_scenario("ruhr-rhine-1991").unwrap();
    assert_eq!(ruhr.hash, data.hash);
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

/// Like `run`, but answers event cards and clears their pause, so a run is never frozen at the
/// first event. (`run` is kept as it was because the pinned Great Lakes hashes were recorded with it.)
fn run_answering(seed: u64, scenario: &str, weeks: u32) -> Session {
    let mut session = Session::create(config(seed, ScenarioKind::Guided, scenario)).unwrap();
    for _ in 0..weeks {
        let result = session.advance(1);
        if result.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.world.pending_pause = None;
        }
    }
    session
}

#[test]
fn silicon_valley_has_its_own_geography_lanes_firms_and_balance() {
    let valley = cw_data::Dataset::load_scenario("silicon-valley-1995").unwrap();
    let lakes = cw_data::Dataset::load().unwrap();
    assert_ne!(valley.hash, lakes.hash);
    assert_eq!(valley.scenario_id, "silicon-valley-1995");
    assert_eq!(valley.states.len(), 1);
    assert_eq!(valley.regions.len(), 16);
    assert!(valley.seed_firms.len() >= 45);
    assert!(valley.validate().is_empty(), "{:?}", valley.validate());
    let n = valley.regions.len();
    assert_eq!(valley.lanes.len(), n * (n - 1) / 2);
    // Oakland, Los Angeles and San Diego are ports on one Pacific sea system.
    let oakland = valley.region_index["CA-OAK"];
    let los_angeles = valley.region_index["CA-LAX"];
    let san_diego = valley.region_index["CA-SDG"];
    assert!(valley.sea_km(oakland, los_angeles).is_some());
    assert!(valley.sea_km(los_angeles, san_diego).is_some());
    assert!(valley.public_ports[oakland as usize]);
    assert_eq!(valley.public_ports.iter().filter(|p| **p).count(), 5);
    // Oil and fuel cheaper, chips dearer and pay lower than the Great Lakes engine units.
    let price = |d: &cw_data::Dataset, id: &str| d.reference_price(d.product_index[id]);
    assert!(price(&valley, "crude_oil") < price(&lakes, "crude_oil"));
    assert!(price(&valley, "components") > price(&lakes, "components"));
    assert!(valley.baseline_weekly_wage_cents[0] < lakes.baseline_weekly_wage_cents[0]);
    assert_eq!(price(&valley, "defense"), price(&lakes, "defense"));
    assert_eq!(valley.recipes.len(), lakes.recipes.len());
    assert_eq!(valley.products.len(), lakes.products.len());
    assert_eq!(valley.player_region, "CA-SJC");
    assert_eq!(valley.player_recipe, "hardware_maker");
    // 1995 county estimates add up to the state: about 31.5 million people.
    let people: i64 = valley.regions.iter().map(|r| r.population()).sum();
    assert!((31_000_000..32_000_000).contains(&people), "{people}");
}

#[test]
fn silicon_valley_opens_the_bakersfield_pipelines() {
    let mut session =
        Session::create(config(1, ScenarioKind::Guided, "silicon-valley-1995")).unwrap();
    assert_eq!(session.dataset.scenario_id, "silicon-valley-1995");
    assert_eq!(session.world.regions.len(), 16);
    let pipeline = session.dataset.logistics.mode_index("pipeline").unwrap();
    session.advance(2);
    let st = &session.world.logistics;
    let bakersfield = session.dataset.region_index["CA-BAK"];
    for destination in ["CA-LAX", "CA-OAK"] {
        let to = session.dataset.region_index[destination];
        let key = cw_sim::economy::logistics::LaneKey::new(bakersfield, to, pipeline);
        let lane = st
            .lanes
            .get(&key)
            .unwrap_or_else(|| panic!("the Bakersfield to {destination} pipeline exists"));
        assert!(lane.cap() > 0);
    }
}

#[test]
fn silicon_valley_electronics_prices_follow_the_chip_cycle() {
    // Components are in the scenario's own family; Great Lakes and Texas have no such family.
    let valley = run_answering(3, "silicon-valley-1995", 1);
    let family = |s: &Session| {
        s.world
            .world_prices
            .families
            .iter()
            .any(|f| f.key == "electronics")
    };
    assert!(family(&valley));
    assert!(!family(&run_answering(3, "great-lakes-2000", 1)));
    assert!(!family(&run_answering(3, "texas-1981", 1)));
}

#[test]
fn silicon_valley_runs_replays_and_saves_on_its_own_dataset() {
    let first = run(7, ScenarioKind::Guided, "silicon-valley-1995", 26);
    assert_eq!(
        first,
        run(7, ScenarioKind::Guided, "silicon-valley-1995", 26)
    );
    assert_ne!(first, run(7, ScenarioKind::Guided, "great-lakes-2000", 26));

    let mut session =
        Session::create(config(8, ScenarioKind::Guided, "silicon-valley-1995")).unwrap();
    session.advance(10);
    let bytes = session.export_save().unwrap();
    let mut loaded = Session::import_save(&bytes).unwrap();
    assert_eq!(loaded.dataset.scenario_id, "silicon-valley-1995");
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
    session.advance(5);
    loaded.advance(5);
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
}

/// A short smoke inside the test suite; `examples/scenario_smoke.rs` runs the full 8 x 104.
#[test]
fn silicon_valley_opens_solvent_and_keeps_the_player_company_alive() {
    for seed in 1..=2u64 {
        let session = run_answering(seed, "silicon-valley-1995", 26);
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

#[test]
fn silicon_valley_posts_go_to_the_california_cast_first() {
    let session = run_answering(2, "silicon-valley-1995", 1);
    let ds = &session.dataset;
    let home = |p: cw_sim::economy::cast::PersonId| ds.cast[p as usize].scenario == "silicon-valley-1995";
    let in_post = |p: Option<cw_sim::economy::cast::PersonId>| p.is_some_and(home);
    let ceos = session
        .world
        .firms
        .iter()
        .filter(|(_, f)| f.archetype.is_some())
        .filter(|(_, f)| in_post(f.ceo))
        .count();
    let others = usize::from(in_post(session.world.macro_state.cycle.chair.person))
        + session
            .world
            .cast
            .leaders
            .iter()
            .filter(|p| in_post(**p))
            .count();
    // Chair and party leaders are filled first; every California person holds some post, and
    // the rest of the rivals are led by the wider cast.
    let held = ds
        .cast
        .iter()
        .filter(|p| p.scenario == "silicon-valley-1995")
        .count();
    assert!(held >= 3);
    assert!(
        ceos + others >= 2,
        "{ceos} CEOs and {others} chair or leaders are Californian"
    );
    assert!(
        session
            .world
            .firms
            .iter()
            .any(|(_, f)| f.archetype.is_some() && f.ceo.is_some())
    );
}
