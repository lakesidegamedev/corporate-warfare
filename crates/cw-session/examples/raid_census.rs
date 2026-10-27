//! Diagnostic: market raids and division spread over five guided years.
fn main() {
    let seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
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
        let world = &session.world;
        let dataset = &session.dataset;
        let raids = world.raids.len();
        let won = world.raids.iter().filter(|r| r.success).count();
        let blocks: u32 = world.raids.iter().map(|r| r.blocks as u32).sum();
        let mut divisions = 0;
        let mut regions = std::collections::BTreeSet::new();
        for (handle, firm) in world.firms.iter() {
            if !firm.operative() {
                continue;
            }
            for d in cw_sim::economy::sectors::divisions_of(world, dataset, handle) {
                divisions += 1;
                regions.insert(d.region);
            }
        }
        println!(
            "seed {seed}: raids {raids} won {won} blocks {blocks} divisions {divisions} regions_used {}/{}",
            regions.len(),
            world.regions.len()
        );
    }
}
