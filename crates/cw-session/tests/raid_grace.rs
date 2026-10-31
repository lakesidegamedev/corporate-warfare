//! A new company is left to find its feet: no rival raids the player's divisions in the opening
//! weeks, in Texas Oil Patch 1981 where the first raid used to land in week two, or elsewhere.

use cw_data::Difficulty;
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::economy::sectors::RAID_GRACE_WEEKS;
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn raids_on_player(scenario: &str, seed: u64, weeks: u32) -> usize {
    let mut session = Session::create(ScenarioConfig {
        seed,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario.to_owned(),
            path: None,
            ..Default::default()
        },
    })
    .unwrap();
    let mut seen = 0;
    let mut count = 0;
    for _ in 0..weeks {
        let result = session.advance(1);
        if result.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
        let world = &session.world;
        let Some(player) = world.investor.primary_group else {
            continue;
        };
        count += world
            .raids
            .iter()
            .skip(seen.min(world.raids.len()))
            .filter(|raid| cw_sim::economy::ownership::same_group(world, raid.defender, player))
            .count();
        seen = world.raids.len();
    }
    count
}

#[test]
fn texas_player_is_not_raided_in_the_opening() {
    for seed in 1..=4 {
        assert_eq!(
            raids_on_player("texas-1981", seed, RAID_GRACE_WEEKS),
            0,
            "seed {seed}"
        );
    }
}

#[test]
fn a_rival_raider_launches_at_most_one_campaign_a_year() {
    // The player issues no commands, so every raid is a rival's. Over three years each raider's
    // campaigns are at least 52 weeks apart, and raiding still happens.
    let mut session = Session::create(ScenarioConfig {
        seed: 3,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    })
    .unwrap();
    for _ in 0..156 {
        if session.advance(1).pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
    }
    let mut by_raider: std::collections::BTreeMap<u64, Vec<u32>> = Default::default();
    for raid in &session.world.raids {
        by_raider
            .entry(raid.attacker.0)
            .or_default()
            .push(raid.tick);
    }
    assert!(!by_raider.is_empty(), "rivals still raid");
    for (raider, ticks) in &by_raider {
        for pair in ticks.windows(2) {
            assert!(
                pair[1] >= pair[0] + 52,
                "raider {raider:x} raided in weeks {} and {}",
                pair[0],
                pair[1]
            );
        }
    }
}
