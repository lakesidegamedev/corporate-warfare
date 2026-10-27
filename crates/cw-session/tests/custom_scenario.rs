//! A custom scenario shapes the opening world and still plays.

use cw_data::{CustomLaws, CustomScenario, Dataset};
use cw_session::{ScenarioConfig, Session};
use cw_sim::state::{ScenarioKind, ScenarioMode, UnionLaw};

fn config() -> ScenarioConfig {
    ScenarioConfig {
        seed: 77,
        scenario: ScenarioKind::Founder,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
    }
}

#[test]
fn custom_numbers_reach_the_world_and_it_plays() {
    let data = Dataset::load().unwrap();
    let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Test");
    custom.difficulty = "hard".into();
    custom.economy = "downturn".into();
    custom.starting_capital_cents = Some(123_000_000);
    custom.rivals[0].enabled = false;
    custom.regions[0].wage_pct = 150;
    custom.laws = Some(CustomLaws {
        corporate_tax_bps: 1_000,
        wage_floor_cents: 40_000,
        merger_review_bps: 5_000,
        unions: "strong".into(),
    });
    let mut session = Session::create_custom(config(), &custom).unwrap();
    assert_eq!(session.world.investor.cash_cents, 123_000_000);
    assert_eq!(session.world.politics.laws.corporate_tax_bps, 1_000);
    assert_eq!(session.world.politics.laws.unions, UnionLaw::Strong);
    let plain = Session::create(config()).unwrap();
    assert_eq!(session.world.firms.len() + 1, plain.world.firms.len());
    assert!(
        session.world.regions[0].labour[0].prevailing_wage_cents
            > plain.world.regions[0].labour[0].prevailing_wage_cents
    );
    session.advance(26);
}

#[test]
fn invalid_custom_scenario_is_refused_with_reasons() {
    let data = Dataset::load().unwrap();
    let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Bad");
    custom.difficulty = "nope".into();
    let error = Session::create_custom(config(), &custom).err().unwrap();
    assert!(error.contains("Difficulty must be"), "{error}");
}
