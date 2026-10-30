//! Balance pass 4 (1.9), on the four scenarios as a player starts them: the treasury follows its
//! cash, and opening share prices sit where the shares trade after the first quarter.
use cw_session::{PriceSetup, ScenarioConfig, Session};

const SCENARIOS: [&str; 4] = [
    "great-lakes-2000",
    "ruhr-rhine-1991",
    "silicon-valley-1995",
    "texas-1981",
];

fn career(scenario: &str, seed: u64) -> Session {
    Session::create(ScenarioConfig {
        seed,
        scenario: cw_sim::state::ScenarioKind::Guided,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario.to_owned(),
            path: None,
            timeline: None,
        },
    })
    .unwrap()
}

fn week(session: &mut Session) {
    session.advance(1);
    for e in session.world.events.pending.iter_mut() {
        e.resolved = true;
    }
    if session.world.pending_pause.is_some() {
        session.dismiss_pause();
    }
}

#[test]
fn the_treasury_stays_within_a_believable_band_over_three_years() {
    for scenario in SCENARIOS {
        let mut s = career(scenario, 5);
        let start = s.world.government.book.cash();
        let (mut low, mut high) = (start, start);
        for _ in 0..156 {
            week(&mut s);
            let cash = s.world.government.book.cash();
            low = low.min(cash);
            high = high.max(cash);
        }
        let end = s.world.government.book.cash();
        // Before this pass a three-year treasury ended at about thirteen times its opening.
        assert!(low >= 0, "{scenario}: never negative");
        assert!(
            high <= start * 6,
            "{scenario}: peaked at {high} from {start}"
        );
        assert!(end >= start / 3, "{scenario}: ended at {end} from {start}");
    }
}

#[test]
fn the_board_opens_near_where_it_trades_a_quarter_later() {
    for scenario in SCENARIOS {
        let mut s = career(scenario, 3);
        let board: Vec<_> = s
            .world
            .firms
            .iter()
            .filter(|(_, f)| f.on_board())
            .map(|(h, _)| h)
            .collect();
        let total = |s: &Session| -> i128 {
            board
                .iter()
                .filter_map(|h| s.world.firm(*h))
                .filter(|f| f.operative())
                .map(|f| f.shares.mark_cents as i128 * f.shares.eligible_votes().max(1) as i128)
                .sum()
        };
        let open = total(&s);
        for _ in 0..52 {
            week(&mut s);
        }
        let year = total(&s);
        // Before this pass the board's value roughly doubled in the first year.
        assert!(
            year * 100 >= open * 85 && year * 100 <= open * 160,
            "{scenario}: the board went from {open} to {year} in a year"
        );
    }
}
