//! The two costliest weekly phases on one world, for A/B runs across builds: market clearing on
//! a copy of the world and the AI's proposals, timed at weeks 150 and 520 of the bench career.
//! Run the before and after builds alternately and compare medians.
//!
//! cargo run --release -p cw-session --example phase_ab
use std::time::Instant;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let config = cw_session::ScenarioConfig {
        seed: 7,
        scenario: cw_sim::state::ScenarioKind::Guided,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: cw_session::PriceSetup {
            scenario_id: "great-lakes-2000".to_string(),
            path: None,
        },
    };
    let mut session = cw_session::Session::create(config).unwrap();
    for week in 1..=520u32 {
        let r = session.advance(1);
        if r.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.world.pending_pause = None;
            session.dismiss_pause();
        }
        if week != 150 && week != 520 {
            continue;
        }
        let (mut clear, mut ai) = (Vec::new(), Vec::new());
        let mut check = 0usize;
        for _ in 0..41 {
            let mut copy = session.world.clone();
            let t = Instant::now();
            cw_sim::economy::markets::clear_week(&mut copy, &session.dataset);
            clear.push(t.elapsed().as_secs_f64() * 1000.0);
            check = check.wrapping_add(copy.revision as usize);
            let t = Instant::now();
            let proposals = cw_ai::propose(&session.world, &session.dataset);
            ai.push(t.elapsed().as_secs_f64() * 1000.0);
            check = check.wrapping_add(proposals.len());
        }
        println!(
            "week {week}: clear_week {:.3} ms, ai propose {:.3} ms (medians of 41, check {check})",
            median(clear),
            median(ai)
        );
    }
    println!("hash {}", session.canonical_hash());
}
