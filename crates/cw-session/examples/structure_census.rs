use std::collections::BTreeMap;
fn main() {
    let seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);
    for seed in 1..=seeds {
        let config = cw_session::ScenarioConfig {
            seed,
            scenario: cw_sim::state::ScenarioKind::Guided,
            mode: cw_sim::state::ScenarioMode::FiveYear,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: cw_session::PriceSetup {
                scenario_id: "great-lakes-2000".to_owned(),
                path: Some(cw_data::PricePath::HistoricalVariance),
            },
        };
        let mut s = cw_session::Session::create(config).unwrap();
        let mut seen: BTreeMap<u64, String> = BTreeMap::new();
        let mut last: BTreeMap<u64, String> = BTreeMap::new();
        for week in 1..=104u32 {
            let r = s.advance(1);
            if r.pause.is_some() {
                for e in s.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                s.dismiss_pause();
            }
            let w = &s.world;
            for (h, f) in w.firms.iter() {
                if f.archetype.is_none() && !w.structure.jvs.iter().any(|j| j.firm == h) { /* player or child */
                }
                let jv = w.structure.jvs.iter().any(|j| j.firm == h);
                let info = format!(
                    "jv={} bonds={} loans={} cash={} debt={} ni13={} camps={} partner_of_jv={}",
                    jv,
                    f.bonds.len(),
                    f.loans.len(),
                    f.book.cash(),
                    f.book.debt(),
                    f.book
                        .recent_weeks()
                        .iter()
                        .map(|x| x.net_income)
                        .sum::<i64>(),
                    w.structure
                        .campaigns
                        .iter()
                        .filter(|c| c.target == h)
                        .map(|c| format!("{}:{}:{}", c.opened, c.demand.key(), c.outcome.key()))
                        .collect::<Vec<_>>()
                        .join("|"),
                    w.structure.jvs.iter().any(|j| j.a == h || j.b == h)
                );
                if f.operative() {
                    last.insert(h.0, format!("{} w{} {}", f.name, week, info));
                } else if matches!(f.status, cw_sim::state::FirmStatus::Dissolved)
                    && !seen.contains_key(&h.0)
                {
                    seen.insert(
                        h.0,
                        format!(
                            "seed {seed} dissolved w{week}: LAST {}",
                            last.get(&h.0).cloned().unwrap_or_default()
                        ),
                    );
                }
            }
        }
        for (_, v) in seen {
            println!("{v}");
        }
        for c in s.world.structure.campaigns.iter() {
            println!(
                "CAMP seed={seed} {} {} {}",
                c.reason.key(),
                c.demand.key(),
                c.outcome.key()
            );
        }
        let t = &s.world.structure.stats;
        println!(
            "STATS seed={seed} bonds={} defaults={} jvs={} ipos={} spin_offs={} carve_outs={} activists={} wins={}",
            t.bonds_issued,
            t.bond_defaults,
            t.jvs_formed,
            t.ipos,
            t.spin_offs,
            t.carve_outs,
            t.activist_events,
            t.activist_wins
        );
    }
}
