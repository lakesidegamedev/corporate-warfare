//! World commodity price paths through the session: modes take effect, a save carries them
//! and the custom scenario field is honoured.

use cw_data::{CustomScenario, Dataset, PricePath};
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::economy::world_prices::PriceMode;
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn config(path: Option<PricePath>, scenario: &str) -> ScenarioConfig {
    ScenarioConfig {
        seed: 31,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario.to_owned(),
            path,
            ..Default::default()
        },
    }
}

/// `advance` can return early, so step until the week is reached.
fn run_to(session: &mut Session, week: u32) {
    while session.world.tick.0 < week {
        let result = session.advance(week - session.world.tick.0);
        session.dismiss_pause();
        assert!(
            result.weeks_advanced > 0,
            "stalled at {}",
            session.world.tick.0
        );
    }
}

fn fuel_price(session: &Session) -> i64 {
    let fuel = session.dataset.product_index["fuel"];
    session
        .world
        .gateways
        .iter()
        .find(|g| g.product == fuel)
        .map(|g| g.sell_price_cents)
        .unwrap()
}

#[test]
fn the_scenario_default_applies_and_the_choice_overrides_it() {
    let default = Session::create(config(None, "texas-1981")).unwrap();
    assert_eq!(
        default.world.world_prices.mode,
        PriceMode::HistoricalVariance
    );
    let great = Session::create(config(None, "great-lakes-2000")).unwrap();
    assert_eq!(great.world.world_prices.mode, PriceMode::Historical);
    let random = Session::create(config(Some(PricePath::Random), "great-lakes-2000")).unwrap();
    assert_eq!(random.world.world_prices.mode, PriceMode::Random);
    let off = Session::create(config(None, "")).unwrap();
    assert_eq!(off.world.world_prices.mode, PriceMode::Off);
}

#[test]
fn historical_oil_moves_gateway_prices_and_off_does_not() {
    let mut priced =
        Session::create(config(Some(PricePath::Historical), "great-lakes-2000")).unwrap();
    let mut off = Session::create(config(None, "")).unwrap();
    let start = fuel_price(&priced);
    assert_eq!(start, fuel_price(&off));
    run_to(&mut priced, 104);
    run_to(&mut off, 104);
    let priced_now = fuel_price(&priced);
    let off_now = fuel_price(&off);
    // Historical oil is below its day-one level two years in; inflation alone lifts the off run.
    assert!(priced_now < off_now * 9 / 10, "{priced_now} vs {off_now}");
}

#[test]
fn a_save_in_the_middle_continues_identically() {
    for path in [
        PricePath::Historical,
        PricePath::HistoricalVariance,
        PricePath::Random,
    ] {
        let mut original = Session::create(config(Some(path), "great-lakes-2000")).unwrap();
        run_to(&mut original, 30);
        let bytes = original.export_save().unwrap();
        let mut loaded = Session::import_save(&bytes).unwrap();
        assert_eq!(
            loaded.world.world_prices.mode,
            original.world.world_prices.mode
        );
        run_to(&mut original, 60);
        run_to(&mut loaded, 60);
        assert_eq!(
            original.canonical_hash(),
            loaded.canonical_hash(),
            "{path:?}"
        );
    }
}

#[test]
fn the_custom_scenario_price_path_takes_effect() {
    let data = Dataset::load().unwrap();
    let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Prices");
    custom.price_path = PricePath::Random;
    let session = Session::create_custom(config(None, ""), &custom).unwrap();
    assert_eq!(session.world.world_prices.mode, PriceMode::Random);
}
