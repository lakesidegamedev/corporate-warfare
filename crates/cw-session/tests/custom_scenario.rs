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

#[test]
fn a_saved_custom_career_reloads_with_its_custom_data() {
    let data = Dataset::load().unwrap();
    let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Reload");
    custom.rivals[0].enabled = false;
    custom.rivals[1].size_pct = 200;
    custom.regions[0].wage_pct = 150;
    custom.regions[1].demand_pct = 60;
    let mut original = Session::create_custom(config(), &custom).unwrap();
    original.advance(8);
    let bytes = original.export_save().unwrap();
    let mut loaded = Session::import_save(&bytes).unwrap();
    assert_eq!(loaded.custom.as_ref(), Some(&custom));
    assert_eq!(
        loaded.dataset.regions[0].wage_index_bps,
        original.dataset.regions[0].wage_index_bps
    );
    assert!(loaded.dataset.regions[0].wage_index_bps > data.regions[0].wage_index_bps);
    assert_eq!(
        loaded.dataset.regions[1].demand_index_bps as i64,
        data.regions[1].demand_index_bps as i64 * 60 / 100
    );
    assert_eq!(
        loaded.dataset.seed_firms.len(),
        original.dataset.seed_firms.len()
    );
    assert_eq!(loaded.dataset.seed_firms.len() + 1, data.seed_firms.len());
    // Both continue identically for a quarter, so nothing falls back to standard data.
    original.advance(13);
    loaded.advance(13);
    assert_eq!(original.canonical_hash(), loaded.canonical_hash());
    // A second save keeps the overrides, and a plain career still saves without any.
    let again = Session::import_save(&loaded.export_save().unwrap()).unwrap();
    assert_eq!(again.custom.as_ref(), Some(&custom));
    let plain = Session::create(config()).unwrap();
    assert!(
        Session::import_save(&plain.export_save().unwrap())
            .unwrap()
            .custom
            .is_none()
    );
}
