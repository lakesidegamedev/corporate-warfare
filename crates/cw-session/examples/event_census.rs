//! Count event cards opened in natural play across seeds. Diagnostic, not a test.
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seeds: u64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(5);
    let weeks: u32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(520);
    let mut counts: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for seed in 1..=seeds {
        let dataset = cw_data::Dataset::load().unwrap();
        let mut world = cw_sim::economy::seed::build(
            &dataset,
            seed,
            cw_sim::state::ScenarioKind::Guided,
            cw_sim::state::ScenarioMode::Sandbox,
        );
        for _ in 0..weeks {
            let proposals = cw_ai::propose(&world, &dataset);
            for proposal in proposals {
                let envelope = cw_sim::commands::CommandEnvelope {
                    command_id_high: 7,
                    command_id_low: world.next_sequence,
                    actor: proposal.actor,
                    expected_revision: world.revision,
                    submitted_tick: world.tick.0,
                    intent: proposal.intent,
                };
                let _ = cw_sim::commands::apply(&mut world, &dataset, &envelope);
            }
            cw_sim::tick::advance(&mut world, &dataset);
            // Answer player cards with the first choice so the clock keeps moving.
            let open: Vec<(u64, String)> = world
                .events
                .pending
                .iter()
                .filter(|event| !event.resolved)
                .map(|event| (event.id, event.card.clone()))
                .collect();
            for (id, card) in open {
                if let Some(card) = dataset.event(&card) {
                    let _ = cw_sim::economy::events_engine::respond(
                        &mut world,
                        &dataset,
                        id,
                        &card.choices[0].id,
                    );
                }
            }
            world.pending_pause = None;
        }
        for event in &world.events.pending {
            let player = event
                .subject_firm
                .map(|firm| cw_sim::economy::events_engine::player_side(&world, firm))
                .unwrap_or(true);
            let row = counts.entry(event.card.clone()).or_insert((0, 0));
            if player { row.0 += 1 } else { row.1 += 1 }
        }
    }
    for card in cw_data::Dataset::load().unwrap().events.iter() {
        let (player, rival) = counts.get(&card.id).copied().unwrap_or((0, 0));
        println!("{:24} player={player:4} rival={rival:4}", card.id);
    }
}
