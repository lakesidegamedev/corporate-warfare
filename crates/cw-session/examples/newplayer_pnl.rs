//! New-player P&L diagnosis: plays a scenario's starting company as a passive player or as an
//! "obvious" new player (tutorial moves, one or two plants in the home sector, default policies)
//! and breaks the controlled firms' weekly profit into revenue and every cost line.
//!
//! cargo run --release -p cw-session --example newplayer_pnl -- [scenario] [passive|obvious] [seeds] [weeks]
use cw_core::ledger::Account;
use cw_session::{Detail, Query, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use std::collections::BTreeMap;

fn answer_events(session: &mut Session) {
    let open: Vec<(u64, String)> = session
        .world
        .events
        .pending
        .iter()
        .filter(|e| !e.resolved)
        .filter_map(|e| {
            let card = session.dataset.event(&e.card)?;
            let choice = card
                .choices
                .iter()
                .find(|c| c.handler == "no_action")
                .or_else(|| card.choices.last())?;
            Some((e.id, choice.id.clone()))
        })
        .collect();
    for (id, choice) in open {
        let _ = cw_sim::economy::events_engine::respond(
            &mut session.world,
            &session.dataset,
            id,
            &choice,
        );
    }
    session.world.pending_pause = None;
}

fn controlled(session: &Session) -> Vec<u64> {
    match session.query(Query::Firms) {
        Detail::Firms(rows) => rows
            .into_iter()
            .filter(|f| f.controlled)
            .map(|f| f.handle)
            .collect(),
        _ => Vec::new(),
    }
}

fn send(session: &mut Session, low: &mut u64, intent: CommandIntent) -> bool {
    let env = CommandEnvelope {
        command_id_high: 0xABCD_0000,
        command_id_low: *low,
        actor: INVESTOR_ACTOR,
        expected_revision: session.revision(),
        submitted_tick: session.world.tick.0,
        intent,
    };
    *low += 1;
    session.submit(env).is_ok()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().unwrap_or_else(|| "texas-1981".to_owned());
    let mode = args.next().unwrap_or_else(|| "passive".to_owned());
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(8);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(52);
    let mut lines: BTreeMap<String, i64> = BTreeMap::new();
    let (mut cash0, mut cash1, mut nw0, mut nw1, mut alive) = (0i64, 0i64, 0i64, 0i64, 0i64);
    let mut ni_total = 0i64;
    let mut quarters = [0i64; 4];
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
                scenario_id: scenario.clone(),
                path: None,
            },
        };
        let mut session = Session::create(config).unwrap();
        let mut low = 1u64;
        let group = session.world.investor.primary_group.unwrap();
        let group_cash = |s: &Session| s.world.firms.get(group).map(|f| f.book.cash()).unwrap_or(0);
        cash0 += group_cash(&session);
        nw0 += cw_sim::economy::seed::investor_net_worth(&session.world, &session.dataset);
        // Home sector: the first plant's recipe and region.
        let (home_recipe, home_region) = {
            let f = session.world.firms.get(group).unwrap();
            let fac = session.world.facilities.get(f.facilities[0]).unwrap();
            (fac.recipe, fac.region)
        };
        let mut seen_txn_floor = 0u64;
        for week in 1..=weeks {
            if mode != "passive" {
                if week == 1 && (mode == "obvious" || mode == "research" || mode == "sensible") {
                    send(
                        &mut session,
                        &mut low,
                        CommandIntent::SetResearchBudget {
                            firm: group,
                            amount_cents: std::env::var("RESEARCH_CENTS")
                                .ok()
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(if mode == "sensible" {
                                    200_000
                                } else {
                                    2_000_000
                                }),
                        },
                    );
                }
                let cash = group_cash(&session);
                if (week == 2 || week == 20)
                    && cash > 100_000_000
                    && (mode == "obvious" || mode == "build" || mode == "sensible")
                {
                    send(
                        &mut session,
                        &mut low,
                        CommandIntent::BuildFacility {
                            firm: group,
                            region: home_region,
                            recipe: home_recipe,
                            blocks: 1,
                        },
                    );
                }
            }
            let r = session.advance(1);
            if r.pause.is_some() {
                answer_events(&mut session);
                session.dismiss_pause();
            }
            let tick = session.world.tick.0;
            for h in controlled(&session) {
                let Some(firm) = session.world.firms.get(cw_core::ids::Handle(h)) else {
                    continue;
                };
                for p in firm.book.journal() {
                    if p.week.0 + 1 != tick && p.week.0 != tick.saturating_sub(1) {
                        continue;
                    }
                    if p.txn < seen_txn_floor {
                        continue;
                    }
                    if !p.account.is_profit_and_loss() {
                        continue;
                    }
                    let sign = if matches!(
                        p.account,
                        Account::Revenue | Account::InterestIncome | Account::OtherIncome
                    ) {
                        -1
                    } else {
                        1
                    };
                    let key = format!("{:?}:{}", p.account, p.memo);
                    *lines.entry(key).or_default() += sign * p.delta;
                }
            }
            let _ = &mut seen_txn_floor;
            let q = ((week - 1) / 13).min(3) as usize;
            let closed = session
                .world
                .firms
                .get(group)
                .and_then(|f| f.book.recent_weeks().last().map(|w| w.net_income));
            if let Some(net) = closed {
                quarters[q] += net;
                ni_total += net;
                let shown = std::env::var("WEEKLY")
                    .ok()
                    .map(|v| v.parse::<u32>().unwrap_or(12));
                if shown.is_some_and(|limit| week <= limit) {
                    print!("{} ", net / 100000);
                }
            }
        }
        if std::env::var("FAC").is_ok() && seed <= 2 {
            let f = session.world.firms.get(group).unwrap();
            for h in &f.facilities {
                let x = session.world.facilities.get(*h).unwrap();
                let r = &session.dataset.recipes[x.recipe as usize];
                println!(
                    "  fac seed{seed} {} region {} blocks {} {:?} started {} completes {} out {} short {} staff {} target {} workers {:?}",
                    r.id,
                    x.region,
                    x.blocks,
                    x.status,
                    x.started.0,
                    x.completes.0,
                    x.last_output.0,
                    x.last_inputs_short,
                    x.last_staff_ratio_bps,
                    x.target_bps,
                    x.workers
                );
            }
        }
        if std::env::var("WEEKLY").is_ok() {
            println!("  <- seed {seed} weekly NI in $100K");
        }
        if session
            .world
            .firms
            .get(group)
            .is_some_and(|f| f.operative())
        {
            alive += 1;
        }
        cash1 += group_cash(&session);
        nw1 += cw_sim::economy::seed::investor_net_worth(&session.world, &session.dataset);
    }
    let n = seeds as i64;
    println!(
        "== {scenario} {mode} {seeds} seeds x {weeks} weeks (group book, mean per seed, dollars)"
    );
    println!(
        "group cash {} -> {}; investor NW {} -> {}; group alive {alive}/{seeds}",
        cash0 / 100 / n,
        cash1 / 100 / n,
        nw0 / 100 / n,
        nw1 / 100 / n
    );
    println!(
        "net income by quarter: {:?}; total {}",
        quarters.map(|q| q / 100 / n),
        ni_total / 100 / n
    );
    let mut rows: Vec<_> = lines.into_iter().collect();
    rows.sort_by_key(|(_, v)| -v.abs());
    for (k, v) in rows.iter().take(80) {
        println!("  {:<44} {:>12}", k, v / 100 / n);
    }
}
