//! Market health harness shared by the `market_health` test and example: runs a scenario for
//! several seeds, and reports per good what was made, sold, imported and exported over the run,
//! how often its corridor price moved, and how often consecutive moves reversed direction.
#![allow(dead_code)]

use std::collections::BTreeMap;

use cw_data::Difficulty;
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::state::{ScenarioKind, ScenarioMode};

pub const SCENARIOS: [&str; 4] = [
    "great-lakes-2000",
    "texas-1981",
    "silicon-valley-1995",
    "ruhr-rhine-1991",
];

/// Weeks skipped before flows and prices are judged: the opening settles in.
pub const WARMUP_WEEKS: usize = 8;

/// A weekly price move counts toward the oscillation metric at this size or more, in basis
/// points of the week's price.
pub const MATERIAL_MOVE_BPS: i64 = 50;

#[derive(Clone, Debug, Default)]
pub struct GoodHealth {
    pub made: i64,
    pub sold: i64,
    pub imported: i64,
    pub exported: i64,
    /// Weeks (over every seed) in which the good was made, sold, or crossed a gateway.
    pub live_weeks: u64,
    pub weeks: u64,
    /// Material weekly price moves, and how many reversed the material move before.
    pub moves: u64,
    pub reversals: u64,
    pub flat_weeks: u64,
    pub price_weeks: u64,
    pub reference_cents: i64,
    pub first_price: i64,
    pub last_price: i64,
    /// Each seed's weekly price series after the warm-up, for diagnosis.
    pub series: Vec<Vec<i64>>,
}

impl GoodHealth {
    pub fn traded(&self) -> i64 {
        self.sold + self.imported + self.exported
    }
    /// Direction reversals of material weekly price moves per 52 weeks: a smooth price has a
    /// few a year, a sawtooth has one every week or two (up to 26 a year).
    pub fn swings_per_year(&self) -> i64 {
        if self.price_weeks == 0 {
            return 0;
        }
        (self.reversals * 52 / self.price_weeks) as i64
    }
    pub fn flat_bps(&self) -> i64 {
        if self.price_weeks == 0 {
            return 0;
        }
        self.flat_weeks as i64 * 10_000 / self.price_weeks as i64
    }
}

pub fn config(seed: u64, scenario: &str) -> ScenarioConfig {
    ScenarioConfig {
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
    }
}

/// Answer open cards like a passive player so a pause never freezes the run.
fn answer_events(session: &mut Session) {
    let open: Vec<(u64, String)> = session
        .world
        .events
        .pending
        .iter()
        .filter(|event| !event.resolved)
        .filter_map(|event| {
            let card = session.dataset.event(&event.card)?;
            let choice = card
                .choices
                .iter()
                .find(|choice| choice.handler == "no_action")
                .or_else(|| card.choices.last())?;
            Some((event.id, choice.id.clone()))
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

pub fn run(scenario: &str, seeds: u64, weeks: u32) -> BTreeMap<String, GoodHealth> {
    let mut out: BTreeMap<String, GoodHealth> = BTreeMap::new();
    for seed in 1..=seeds {
        let mut session = Session::create(config(seed, scenario)).unwrap();
        let mut series: BTreeMap<String, Vec<i64>> = BTreeMap::new();
        for week in 1..=weeks {
            let result = session.advance(1);
            if result.pause.is_some() {
                answer_events(&mut session);
                session.dismiss_pause();
            }
            let board = cw_session::dto::commodity_board(&session.world, &session.dataset);
            for good in &board {
                let row = out.entry(good.product_id.clone()).or_default();
                row.reference_cents = good.reference_cents;
                series
                    .entry(good.product_id.clone())
                    .or_default()
                    .push(good.price_cents);
                if (week as usize) <= WARMUP_WEEKS {
                    continue;
                }
                row.made += good.made_lots;
                row.sold += good.sold_lots;
                row.imported += good.imported_lots;
                row.exported += good.exported_lots;
                row.weeks += 1;
                if good.made_lots + good.sold_lots + good.imported_lots + good.exported_lots > 0 {
                    row.live_weeks += 1;
                }
            }
        }
        for (id, prices) in series {
            let row = out.get_mut(&id).unwrap();
            let prices = &prices[WARMUP_WEEKS.min(prices.len())..];
            row.series.push(prices.to_vec());
            row.first_price = prices.first().copied().unwrap_or(0);
            row.last_price = prices.last().copied().unwrap_or(0);
            let mut last_sign = 0i64;
            for pair in prices.windows(2) {
                row.price_weeks += 1;
                let delta = pair[1] - pair[0];
                if delta == 0 {
                    row.flat_weeks += 1;
                }
                // Cent-level jitter is not a swing: only moves of at least half a percent count.
                if delta.abs() * 10_000 < pair[0] * MATERIAL_MOVE_BPS {
                    continue;
                }
                let sign = delta.signum();
                row.moves += 1;
                if last_sign != 0 && sign != last_sign {
                    row.reversals += 1;
                }
                last_sign = sign;
            }
        }
    }
    out
}

pub fn render(
    scenario: &str,
    seeds: u64,
    weeks: u32,
    rows: &BTreeMap<String, GoodHealth>,
) -> String {
    let mut text = format!("scenario {scenario}: {seeds} seeds x {weeks} weeks\n");
    text += &format!(
        "{:24} {:>9} {:>9} {:>9} {:>9} {:>6} {:>6} {:>8}\n",
        "good", "made", "sold", "import", "export", "swing", "flat%", "price"
    );
    for (id, g) in rows {
        text += &format!(
            "{:24} {:>9} {:>9} {:>9} {:>9} {:>6} {:>6} {:>8}\n",
            id,
            g.made,
            g.sold,
            g.imported,
            g.exported,
            g.swings_per_year(),
            g.flat_bps() / 100,
            format!("{}/{}", g.last_price / 100, g.reference_cents / 100),
        );
    }
    text
}
