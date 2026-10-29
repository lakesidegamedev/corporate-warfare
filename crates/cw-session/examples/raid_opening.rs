//! Diagnostic: raids against the player's group in the opening weeks of each scenario.
//!
//! cargo run --release -p cw-session --example raid_opening -- [seeds] [weeks] [verbose]
fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(8);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(26);
    let verbose = args.next().is_some();
    for scenario in [
        "texas-1981",
        "great-lakes-2000",
        "silicon-valley-1995",
        "ruhr-rhine-1991",
    ] {
        let mut total = 0;
        let mut lost = 0;
        let mut first: Vec<u32> = Vec::new();
        for seed in 1..=seeds {
            let config = cw_session::ScenarioConfig {
                seed,
                scenario: cw_sim::state::ScenarioKind::Guided,
                mode: cw_sim::state::ScenarioMode::Sandbox,
                dataset_hash: String::new(),
                difficulty: cw_data::Difficulty::Standard,
                economy_start: Default::default(),
                starting_capital_cents: None,
                prices: cw_session::PriceSetup {
                    scenario_id: scenario.to_owned(),
                    path: None,
                    ..Default::default()
                },
            };
            let mut session = match cw_session::Session::create(config) {
                Ok(s) => s,
                Err(e) => {
                    println!("{scenario}: create failed {e:?}");
                    break;
                }
            };
            let mut seen = 0;
            for _ in 0..weeks {
                let r = session.advance(1);
                if r.pause.is_some() {
                    for e in session.world.events.pending.iter_mut() {
                        e.resolved = true;
                    }
                    session.dismiss_pause();
                }
                let w = &session.world;
                let Some(player) = w.investor.primary_group else {
                    continue;
                };
                for raid in w.raids.iter().skip(seen.min(w.raids.len())) {
                    if cw_sim::economy::ownership::same_group(w, raid.defender, player) {
                        total += 1;
                        lost += raid.success as i32;
                        first.push(raid.tick);
                        if verbose {
                            let name =
                                |h| w.firms.get(h).map(|f| f.name.clone()).unwrap_or_default();
                            let temper = format!(
                                "{:?}",
                                cw_sim::economy::cast::temperament(
                                    w,
                                    &session.dataset,
                                    raid.attacker
                                )
                            );
                            println!(
                                "  seed {seed} wk {} {} -> {} budget {} chance {} ok {} blocks {} {temper}",
                                raid.tick,
                                name(raid.attacker),
                                name(raid.defender),
                                raid.budget_cents / 100,
                                raid.chance_bps,
                                raid.success,
                                raid.blocks
                            );
                        }
                    }
                }
                seen = w.raids.len();
            }
        }
        println!(
            "{scenario}: raids on player {total} (won {lost}) in {weeks} weeks x {seeds} seeds; ticks {first:?}"
        );
    }
}
