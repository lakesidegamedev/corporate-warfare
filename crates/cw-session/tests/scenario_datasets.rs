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
            ..Default::default()
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
/// chief executives from it, and again when rivals began choosing moves by temperament with
/// grudges and alliances (1.7 preview 5), and again in 1.7.1 when new divisions gained a raid
/// grace period, a plant under construction stopped hiring its crew until the last four weeks,
/// research stopped spending at the frontier, seed share prices began from an opening earnings
/// prior and the world began carrying its scenario start year, and again when every good gained a
/// producer in every scenario (new maintenance strategy and opening plants), every firm priced
/// weekly on a damped rule and services followed demand, and again in 1.8 preview 3 when goods
/// prices began rising on buyers' unmet input cover instead of the seller's empty post-trade
/// shelf, services priced on spare installed capacity, import-dependent goods priced near the
/// import, rivals weighed replacing imports and carriers cut below 60 percent lane fill, and again
/// in the same preview when refineries began shifting their cut between fractions, co-products
/// gained export room, household unmet demand began lifting prices and facilities, households
/// and policies gained new encoded fields, and again in the same preview when carriers began
/// trimming toward 85 percent fill by lot-km unless their lanes overflow and rival raiders began
/// waiting a year between campaigns, and again in the same preview when gateways began adding
/// export room after weeks their exports filled them and idle rivals began winding down, and
/// again in the same preview when firms began feeding their own plants from their stock in other
/// states, carved-out divisions began taking their contracts, and contracts began shipping from
/// where the seller makes the good, and again when freight rates and service references began
/// following the price index, rivals began sizing pipelines on their own lane shipments, and
/// carve-outs began ending purchases nothing in the state uses, and in 1.9 when save schema 3
/// added the scenario timeline cursor (four empty bytes in every world) and Great Lakes began
/// playing its dated history by default (inside the first 52 weeks: the technology bust in week 15
/// and the notice of the 2001 recession), and in 1.9 when rival raid campaigns began to be paid
/// out of earnings and rivals stopped building for imports that a plant under construction
/// already covers (the fixture path pinned below does not move: neither rule bites in 26 weeks),
/// and in 1.9 when scenarios began opening with the rates and inflation of their own period (Great
/// Lakes 2000: inflation 3.4 percent not 3, policy rate 6.5 percent not 6, expected inflation and
/// opening loan rates to match; this moves the fixture path too, from the first week), and in 1.9
/// when companies gained executives (rivals hire a chief financial, operating or logistics officer
/// when it pays, which changes their loans, builds and freight inside the first year, and the pool
/// of people to hire is drawn in week one and hashed), and in 1.9 when the outside account began
/// selling on its stock to the rest of the world a thirteenth a week (from week two, so the
/// fixture path moves), rivals began closing idle carriers and netting carrier builds, and a
/// restructuring began mothballing plants that lose money and lasting its full year.
#[test]
fn great_lakes_dataset_and_worlds_are_unchanged() {
    let data = cw_data::Dataset::load().unwrap();
    assert_eq!(data.scenario_id, "great-lakes-2000");
    assert_eq!(
        data.hash,
        "51e3b2ad8be01df2f07096fbd80e476005521df50f1352704f1db3a7b901d68e"
    );
    let pins = [
        (
            1u64,
            "b9b828b37e4ee51acd7b2f6d7516f4eb9b550e7798c7ccca41541a1398e3fada",
        ),
        (
            2,
            "202352be15716292a686f5e6780e62d16de708522c8ec9f6f1022b2331b24cea",
        ),
        (
            3,
            "e2044d285568723a61722116f9d903164e0a12f0b469fd3f5170af4f09f56410",
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
        "8c9227aa8450ae72371aec0f116a7df96e76d8c3d08816cb55b804c3a5b34240"
    );
    // No world prices and no scenario named: the engine fixture path.
    assert_eq!(
        run(5, ScenarioKind::Guided, "", 26),
        "ff248003914536bf43555d8cd2e97ce900275f8fb429a84064d589d8c1e06596"
    );
    // A scenario with no content of its own plays on the default dataset.
    let unlisted = cw_data::Dataset::load_scenario("atlantis-2050").unwrap();
    assert_eq!(unlisted.hash, data.hash);
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
    let home =
        |p: cw_sim::economy::cast::PersonId| ds.cast[p as usize].scenario == "silicon-valley-1995";
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

/// Ruhr and Rhine 1991 is the first scenario with more than one money.
#[test]
fn ruhr_rhine_has_five_countries_four_currencies_and_its_own_geography() {
    let ruhr = cw_data::Dataset::load_scenario("ruhr-rhine-1991").unwrap();
    let lakes = cw_data::Dataset::load().unwrap();
    assert_ne!(ruhr.hash, lakes.hash);
    assert!(ruhr.validate().is_empty(), "{:?}", ruhr.validate());
    assert!((16..=20).contains(&ruhr.regions.len()));
    let set = ruhr.currencies.as_ref().expect("currencies");
    assert_eq!(set.currencies.len(), 4);
    assert_eq!(set.currencies[0].id, "DEM", "the numeraire comes first");
    assert_eq!(ruhr.region_currency.len(), ruhr.regions.len());
    // Belgium and Luxembourg share the franc; Germany keeps the mark.
    let money = |code: &str| {
        set.currencies[ruhr.region_currency[ruhr.region_index[code] as usize] as usize]
            .id
            .clone()
    };
    assert_eq!(money("DE-RUH"), "DEM");
    assert_eq!(money("NL-ROT"), "NLG");
    assert_eq!(money("BE-ANT"), "BEF");
    assert_eq!(money("LU-LUX"), "BEF");
    assert_eq!(money("FR-LOR"), "FRF");
    assert_eq!(ruhr.player_region, "DE-RUH");
    // The single-money scenarios carry no currency data at all.
    assert!(lakes.currencies.is_none() && lakes.region_currency.is_empty());
    assert!(
        cw_data::Dataset::load_scenario("texas-1981")
            .unwrap()
            .currencies
            .is_none()
    );
}

#[test]
fn ruhr_rhine_is_deterministic_saves_with_its_rates_and_replays() {
    let first = run(11, ScenarioKind::Guided, "ruhr-rhine-1991", 30);
    assert_eq!(first, run(11, ScenarioKind::Guided, "ruhr-rhine-1991", 30));

    let mut session = Session::create(config(12, ScenarioKind::Guided, "ruhr-rhine-1991")).unwrap();
    session.advance(12);
    assert!(session.world.fx.active);
    let bytes = session.export_save().unwrap();
    let mut loaded = Session::import_save(&bytes).unwrap();
    assert!(
        loaded.world.fx.active,
        "the rate model came back from the save"
    );
    assert_eq!(loaded.world.fx, session.world.fx);
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
    session.advance(8);
    loaded.advance(8);
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());

    let log: cw_session::ReplayLog = serde_json::from_str(&session.export_replay()).unwrap();
    assert_eq!(Session::replay(&log).unwrap(), session.canonical_hash());
}

/// Money paid across a border is accounted per currency: what buyers paid in is either delivered
/// to a seller or still an open receivable, every ledger balances, and the recorded foreign
/// exchange results are exactly the receivable differences.
#[test]
fn ruhr_rhine_cross_border_payments_reconcile_per_currency() {
    let mut session = Session::create(config(13, ScenarioKind::Guided, "ruhr-rhine-1991")).unwrap();
    for _ in 0..52 {
        let result = session.advance(1);
        if result.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
    }
    let world = &session.world;
    let mut open = vec![0i64; world.fx.currencies.len()];
    for receivable in &world.fx.receivables {
        open[receivable.from as usize] += receivable.amount;
    }
    let mut paid_across = 0;
    for (index, currency) in world.fx.currencies.iter().enumerate() {
        assert_eq!(
            currency.inflow - currency.outflow,
            open[index],
            "currency {index}"
        );
        paid_across += currency.inflow;
    }
    assert!(paid_across > 0, "firms in different countries traded");
    for (_, firm) in world.firms.iter() {
        assert!(firm.book.is_balanced());
    }
    // Each firm's recorded gain less loss equals its fx postings in other income.
    for (handle, pnl) in &world.fx.pnl {
        let Some(firm) = world.firms.get(*handle) else {
            continue;
        };
        let posted: i64 = firm
            .book
            .journal()
            .iter()
            .filter(|p| p.memo == "fx gain" || p.memo == "fx loss")
            .map(|p| -p.delta)
            .sum();
        // The journal keeps thirteen weeks, so it can only hold a part of the lifetime total.
        assert!(posted.abs() <= (pnl.gains + pnl.losses));
    }
}

#[test]
fn ruhr_rhine_reporting_currency_changes_the_figures_not_the_world() {
    let mut session = Session::create(config(14, ScenarioKind::Guided, "ruhr-rhine-1991")).unwrap();
    session.advance(30);
    let before = session.canonical_hash();
    let in_marks = session.overview();
    assert_eq!(in_marks.reporting_currency, "DEM");
    assert!(session.set_reporting_currency("FRF"));
    assert!(!session.set_reporting_currency("USD"));
    let in_francs = session.overview();
    assert_eq!(in_francs.reporting_currency, "FRF");
    assert_eq!(
        session.canonical_hash(),
        before,
        "a display choice never moves the world"
    );
    let francs = session
        .dataset
        .currencies
        .as_ref()
        .unwrap()
        .index_of("FRF")
        .unwrap() as u8;
    let expected = session
        .world
        .fx
        .convert(in_marks.investor_net_worth_cents, 0, francs);
    assert!((in_francs.investor_net_worth_cents - expected).abs() <= 1);
    let view = session.fx_view().expect("an fx view");
    assert_eq!(view.reporting, "FRF");
    assert_eq!(view.currencies.len(), 4);
    // Every central bank has a named chair from the cast, and no two banks share one.
    let mut chairs: Vec<&str> = view
        .currencies
        .iter()
        .map(|c| c.chair_name.as_str())
        .collect();
    assert!(chairs.iter().all(|name| !name.is_empty()), "{chairs:?}");
    chairs.sort_unstable();
    chairs.dedup();
    assert_eq!(chairs.len(), 4, "four banks, four chairs");
}

#[test]
fn ruhr_rhine_opens_solvent_and_keeps_the_player_company_alive() {
    for seed in 1..=2u64 {
        let mut session =
            Session::create(config(seed, ScenarioKind::Guided, "ruhr-rhine-1991")).unwrap();
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
    }
}

const SCENARIOS: [(&str, i32); 4] = [
    ("great-lakes-2000", 2000),
    ("texas-1981", 1981),
    ("silicon-valley-1995", 1995),
    ("ruhr-rhine-1991", 1991),
];

#[test]
fn every_scenario_opens_in_its_own_year() {
    for (scenario, year) in SCENARIOS {
        let mut session = Session::create(config(1, ScenarioKind::Guided, scenario)).unwrap();
        for _ in 0..5 {
            session.advance(1);
        }
        let overview = session.overview();
        assert_eq!(overview.start_year, year, "{scenario}");
        assert_eq!(overview.week_label, format!("{year} W05"), "{scenario}");
        // The year turns over after 52 weeks, not before.
        for _ in 5..53 {
            session.advance(1);
            if session.world.events.pending.iter().any(|e| !e.resolved) {
                for event in session.world.events.pending.iter_mut() {
                    event.resolved = true;
                }
                session.dismiss_pause();
            }
        }
        assert!(
            session
                .overview()
                .week_label
                .starts_with(&format!("{} ", year + 1)),
            "{scenario}: {}",
            session.overview().week_label
        );
    }
}

/// Share price over fair value, for firms worth more than a few dollars a share.
fn price_ratios(session: &Session) -> Vec<(String, f64)> {
    session
        .world
        .firms
        .iter()
        .filter(|(_, f)| f.shares.fair_cents >= 100)
        .map(|(_, f)| {
            (
                f.name.clone(),
                f.shares.mark_cents as f64 / f.shares.fair_cents as f64,
            )
        })
        .collect()
}

/// A seed firm's first quarter of earnings is unobserved, so its fair value must not collapse
/// away from the opening price faster than the price can follow it.
#[test]
fn opening_share_prices_stay_near_fair_value() {
    for (scenario, _) in SCENARIOS {
        for seed in [1, 2] {
            let mut session =
                Session::create(config(seed, ScenarioKind::Guided, scenario)).unwrap();
            for week in 1..=52u32 {
                let result = session.advance(1);
                if result.pause.is_some() {
                    for event in session.world.events.pending.iter_mut() {
                        event.resolved = true;
                    }
                    session.dismiss_pause();
                }
                if week == 5 || week == 52 {
                    // By week 52 a firm that has since become very profitable can sit well under its
                    // new fair value while the price catches up. Since 1.7.1 firms read their real
                    // weekly sales from the first week, so early prices rise toward what sells out
                    // and the median can open up to a fifth under fair value instead of at it.
                    let (low, high) = if week == 5 { (0.3, 4.0) } else { (0.08, 6.0) };
                    let ratios = price_ratios(&session);
                    let mut sorted: Vec<f64> = ratios.iter().map(|r| r.1).collect();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let median = sorted[sorted.len() / 2];
                    assert!(
                        (0.8..=1.3).contains(&median),
                        "{scenario} seed {seed} week {week}: median {median}"
                    );
                    for (name, ratio) in ratios {
                        assert!(
                            (low..=high).contains(&ratio),
                            "{scenario} seed {seed} week {week}: {name} price/fair {ratio}"
                        );
                    }
                }
            }
        }
    }
}
