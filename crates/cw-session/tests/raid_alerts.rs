//! What the player is told about raids. Oracle: the raids and plants are set by hand, so the
//! expected words and figures are read straight off what was set.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_session::{ScenarioConfig, Session};
use cw_sim::state::{RaidRecord, ScenarioKind, ScenarioMode};

fn session() -> Session {
    Session::create(ScenarioConfig {
        seed: 52,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    })
    .unwrap()
}

fn rival(s: &Session, me: Handle) -> Handle {
    s.world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| *h != me && !cw_sim::economy::events_engine::player_side(&s.world, *h))
        .unwrap()
}

fn one_block(s: &mut Session, me: Handle) -> (u8, u8) {
    let plants = s.world.firm(me).unwrap().facilities.clone();
    for (i, handle) in plants.iter().enumerate() {
        if i > 0 {
            s.world.facilities.remove(*handle);
        }
    }
    s.world.firms.get_mut(me).unwrap().facilities.truncate(1);
    let plant = s.world.facilities.get_mut(plants[0]).unwrap();
    plant.blocks = 1;
    plant.started = Tick(0);
    let industry = cw_sim::economy::sectors::recipe_industry(&s.dataset, plant.recipe);
    (plant.region, industry)
}

fn raid(s: &Session, me: Handle, region: u8, industry: u8, success: bool) -> RaidRecord {
    RaidRecord {
        tick: s.world.tick.0,
        attacker: rival(s, me),
        defender: me,
        region,
        industry,
        budget_cents: 5_000_000,
        chance_bps: 1_300,
        success,
        blocks: 1,
        price_cents: 150_000,
    }
}

fn detail(s: &Session, key: &str) -> Option<String> {
    s.overview()
        .alerts
        .into_iter()
        .find(|a| a.key == key)
        .map(|a| a.detail)
}

#[test]
fn a_one_block_business_is_warned_before_any_raid() {
    let mut s = session();
    let me = s.world.investor.primary_group.unwrap();
    one_block(&mut s, me);
    s.world.tick = Tick(40);
    let text = detail(&s, "alert.raid_exposed").expect("a warning");
    assert!(text.contains("single block"), "{text}");
    assert!(
        text.contains("percent chance of taking all of it"),
        "{text}"
    );
    assert!(text.contains("Raising weekly marketing by"), "{text}");
    // Inside the opening grace there is nothing to warn about.
    s.world.tick = Tick(5);
    assert!(detail(&s, "alert.raid_exposed").is_none());
}

#[test]
fn a_raid_on_you_names_the_raider_the_chance_and_the_defence() {
    let mut s = session();
    let me = s.world.investor.primary_group.unwrap();
    let (region, industry) = one_block(&mut s, me);
    s.world.tick = Tick(40);
    let record = raid(&s, me, region, industry, false);
    let raider = s.world.firm(record.attacker).unwrap().name.clone();
    s.world.raids.push(record);
    let text = detail(&s, "alert.raid").expect("an alert");
    assert!(text.contains(&raider) && text.contains("failed"), "{text}");
    assert!(text.contains("13.00 percent"), "{text}");
    assert!(text.contains("cuts their chance from"), "{text}");
    // Four weeks on it is no longer news.
    s.world.tick = Tick(45);
    assert!(detail(&s, "alert.raid").is_none());
}

#[test]
fn losing_the_last_block_of_the_only_business_is_said_plainly_with_ways_on() {
    let mut s = session();
    let me = s.world.investor.primary_group.unwrap();
    let (region, industry) = one_block(&mut s, me);
    s.world.tick = Tick(40);
    let record = raid(&s, me, region, industry, true);
    let raider = s.world.firm(record.attacker).unwrap().name.clone();
    // The raid took the plant: it is gone from the company.
    for handle in s.world.firm(me).unwrap().facilities.clone() {
        s.world.facilities.remove(handle);
    }
    s.world.firms.get_mut(me).unwrap().facilities.clear();
    s.world.raids.push(record);
    let text = detail(&s, "alert.raid_last_block").expect("the last block alert");
    assert!(
        text.contains(&format!("{raider} took the last block")),
        "{text}"
    );
    assert!(
        text.contains("build a new plant") && text.contains("found a new company"),
        "{text}"
    );
    let won = detail(&s, "alert.raid").expect("the raid alert");
    assert!(won.contains("1 block taken"), "{won}");
    let letters = cw_session::letters::letters(&s.world, &s.dataset, 0);
    assert!(letters.iter().any(
        |l| l.id.starts_with("raid:") && l.paragraphs.join(" ").contains("last of your plants")
    ));
    for t in [&text, &won] {
        assert!(!t.contains('\u{2014}') && !t.contains('\u{2013}'));
    }
}
