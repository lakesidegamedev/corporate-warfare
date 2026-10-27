//! Diagnostic: which industries and archetypes dissolve over five years, and why.
use std::collections::BTreeMap;

fn main() {
    let seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let mut tally: BTreeMap<(String, String), (u32, u32)> = BTreeMap::new();
    for seed in 1..=seeds {
        let config = cw_session::ScenarioConfig {
            seed,
            scenario: cw_sim::state::ScenarioKind::Guided,
            mode: cw_sim::state::ScenarioMode::Sandbox,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: Default::default(),
        };
        let mut session = cw_session::Session::create(config).unwrap();
        for _ in 0..260 {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
        }
        for (_, firm) in session.world.firms.iter() {
            let Some(archetype) = firm.archetype else {
                continue;
            };
            let industry = cw_sim::state::INDUSTRY_NAMES[firm.industry as usize].to_owned();
            let row = tally
                .entry((industry, archetype.key().to_owned()))
                .or_insert((0, 0));
            row.0 += 1;
            if matches!(firm.status, cw_sim::state::FirmStatus::Dissolved) {
                row.1 += 1;
            }
        }
    }
    for ((industry, archetype), (firms, dissolved)) in tally {
        println!("{industry:22} {archetype:14} firms={firms:3} dissolved={dissolved:3}");
    }
}
