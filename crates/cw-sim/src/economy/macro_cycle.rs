//! The business cycle and the central bank, run every week.
//!
//! Output runs above or below its trend with momentum and small random shocks. Cheap money and
//! good times build credit; the more credit, the likelier a bust, which knocks the economy down
//! and leaves borrowers paying debt off for months. Inflation drifts toward expected inflation
//! plus a share of the output gap. The central bank meets every six weeks and moves its rate a
//! quarter point at a time (half a point when far off) toward a Taylor rule. The cycle reaches
//! the rest of the game through household demand, loan rates and a nominal drift in background
//! wages and gateway prices.

use crate::state::{CyclePhase, MACRO_HISTORY_WEEKS, MacroPoint, RegimeKind, World};
use cw_core::calendar::Tick;
use cw_data::Dataset;

/// Weeks between central bank meetings.
pub const MEETING_WEEKS: u32 = 6;
const RATE_FLOOR_BPS: i32 = 25;
const RATE_CEILING_BPS: i32 = 1_500;

/// Advance the economy one week and apply its effects.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    let week = world.tick.0;
    let rate = world.macro_state.policy_rate_bps;
    let inflation = world.macro_state.inflation_bps;
    let shock = world.macro_state.rng.uniform_inclusive(-20, 20) as i32;
    let bust_roll = world.macro_state.rng.uniform_below(10_000) as i32;

    let c = &mut world.macro_state.cycle;
    let real = rate - inflation;
    let mut momentum = c.momentum_bps * 93 / 100 + shock - (real - c.neutral_real_bps) / 70
        + c.credit_bps / 120
        - c.output_gap_bps * 2 / 100
        - c.deleveraging_bps / 60;
    let built = if c.output_gap_bps > 0 {
        c.output_gap_bps / 40
    } else {
        c.output_gap_bps / 60
    };
    c.credit_bps = (c.credit_bps + built + (c.neutral_real_bps - real) / 50 + 1).clamp(0, 1_500);
    c.deleveraging_bps = c.deleveraging_bps * 95 / 100;
    let hazard = (c.credit_bps - 450).max(0) / 3;
    if bust_roll < hazard {
        momentum -= 180;
        c.deleveraging_bps += c.credit_bps / 2;
        c.credit_bps = c.credit_bps * 35 / 100;
    }
    c.momentum_bps = momentum.clamp(-160, 100);
    let gap_change = c.momentum_bps / 5;
    c.output_gap_bps = (c.output_gap_bps + gap_change).clamp(-700, 600);
    c.potential_index +=
        (c.potential_index as i128 * c.potential_growth_bps as i128 / (52 * 10_000)) as i64;
    c.gdp_index = c.potential_index * (10_000 + c.output_gap_bps as i64) / 10_000;

    // Inflation moves toward expected inflation plus a quarter of the gap; expectations follow
    // inflation slowly and are anchored by the target.
    let pull = (c.expected_inflation_bps + c.output_gap_bps * 25 / 100 - inflation) / 12;
    let new_inflation = (inflation + pull).clamp(-300, 2_000);
    c.expected_inflation_bps += (new_inflation - c.expected_inflation_bps) / 40
        + (c.target_inflation_bps - c.expected_inflation_bps) / 60;

    // The central bank meets on its calendar. The chair's stance tilts the rule: a hawk weighs
    // inflation more, output less, and runs a little tighter.
    let mut new_rate = rate;
    let mut decision = None;
    if week >= c.next_meeting {
        c.next_meeting = week + MEETING_WEEKS;
        let stance = c.chair.stance;
        let inflation_gap = new_inflation - c.target_inflation_bps;
        let taylor = c.neutral_real_bps
            + new_inflation
            + inflation_gap * (50 + stance / 2) / 100
            + c.output_gap_bps * (50 - stance * 3 / 10) / 100
            + stance / 2;
        let target = (taylor + 12).div_euclid(25) * 25;
        let distance = target - rate;
        let step = if distance.abs() < 25 {
            0
        } else if distance.abs() >= 100 {
            50
        } else {
            25
        };
        new_rate = (rate + step * distance.signum()).clamp(RATE_FLOOR_BPS, RATE_CEILING_BPS);
        c.last_decision_bps = new_rate - rate;
        let reason = if new_rate > rate {
            if inflation_gap > 50 {
                "inflation running above target"
            } else {
                "an economy running hot"
            }
        } else if new_rate < rate {
            if c.output_gap_bps < -100 {
                "a weakening economy"
            } else {
                "inflation back near target"
            }
        } else if inflation_gap > 50 {
            "watching inflation, holding for now"
        } else {
            "the outlook balanced"
        };
        decision = Some((new_rate - rate, new_rate, reason));
    }
    if let Some((change, rate_now, reason)) = decision {
        c.decisions.push(crate::state::RateDecision {
            week,
            change_bps: change,
            rate_bps: rate_now,
            reason: reason.to_owned(),
        });
        let excess = c.decisions.len().saturating_sub(30);
        c.decisions.drain(..excess);
    }

    // Phase, with hysteresis so a recession is not flagged on and off week to week.
    let previous = c.phase;
    let gap = c.output_gap_bps;
    let m = c.momentum_bps;
    c.phase = if previous == CyclePhase::Recession {
        if gap > -30 || m > 15 {
            CyclePhase::Recovery
        } else {
            CyclePhase::Recession
        }
    } else if gap < -100 && m <= 0 {
        CyclePhase::Recession
    } else if gap < 0 && m > 0 {
        CyclePhase::Recovery
    } else if gap >= 250 {
        CyclePhase::Boom
    } else if gap >= 0 && m < 0 {
        CyclePhase::Slowdown
    } else {
        CyclePhase::Expansion
    };
    let phase = c.phase;
    c.history.push(MacroPoint {
        week,
        gdp_index: c.gdp_index,
        output_gap_bps: c.output_gap_bps,
        inflation_bps: new_inflation,
        policy_rate_bps: new_rate,
        phase,
    });
    let excess = c.history.len().saturating_sub(MACRO_HISTORY_WEEKS);
    c.history.drain(..excess);
    let growth = year_on_year_growth(&c.history);

    world.macro_state.inflation_bps = new_inflation;
    rotate_chair(world, dataset);
    world.macro_state.policy_rate_bps = new_rate;
    world.macro_state.real_growth_bps = growth;
    apply_nominal_drift(world, new_inflation);
    apply_demand(world, gap);

    // Tell the player when the economy turns: the same cards the old regime draw used.
    if phase != previous {
        let card = match phase {
            CyclePhase::Recession => Some(("recession", RegimeKind::Recession)),
            CyclePhase::Boom => Some(("demand_boom", RegimeKind::Boom)),
            _ => None,
        };
        if let Some((card, kind)) = card
            && dataset.events.iter().any(|event| event.id == card)
        {
            world.macro_state.regime = Some((kind, Tick(week + 26)));
            let subject = world
                .investor
                .primary_group
                .filter(|handle| world.firms.contains(*handle));
            crate::economy::events_engine::open(world, card, subject, None, false);
        }
    }
}

/// At the end of a term the government reappoints the chair or picks a new one from the cast.
/// A pro-business government leans toward a dovish chair, a pro-labour one toward a centrist;
/// a successful chair (inflation near target, no recession) is likelier to stay. The new chair
/// is the free chair-capable person whose stance is nearest the government's draw, preferring
/// the scenario's own people; with nobody free the incumbent stays.
fn rotate_chair(world: &mut World, dataset: &Dataset) {
    let week = world.tick.0;
    if week < world.macro_state.cycle.chair.term_ends {
        return;
    }
    let inflation = world.macro_state.inflation_bps;
    let recession = world.macro_state.cycle.phase == CyclePhase::Recession;
    let party = world.politics.party;
    let rng = &mut world.macro_state.rng;
    let keep_bps =
        if (inflation - world.macro_state.cycle.target_inflation_bps).abs() < 100 && !recession {
            6_000
        } else {
            2_500
        };
    let keep = rng.chance_bps(keep_bps);
    let lean = match party {
        crate::state::Party::Business => -20,
        crate::state::Party::Labour => 0,
    };
    let desired = (rng.uniform_inclusive(-70, 70) as i32 + lean).clamp(-100, 100);
    let successor = if keep {
        None
    } else {
        crate::economy::cast::pick_chair(world, dataset, desired)
    };
    let c = &mut world.macro_state.cycle;
    match successor {
        None => {
            c.chair.term_ends = week + 208;
            let person = c.chair.person;
            let text = format!(
                "{} reappointed to chair the central bank for four more years.",
                c.chair.name
            );
            world
                .politics
                .news
                .push(crate::state::PoliticalNews { person, week, text });
        }
        Some(person) => {
            let appointed = week;
            c.chair = crate::state::CentralBanker {
                name: String::new(),
                stance: 0,
                appointed_week: appointed,
                term_ends: week + 208,
                person: None,
            };
            crate::economy::cast::seat_chair(dataset, &mut c.chair, person);
            let text = format!(
                "{} takes over the central bank, seen as {}.",
                c.chair.name,
                c.chair.stance_label()
            );
            world.politics.news.push(crate::state::PoliticalNews {
                person: Some(person),
                week,
                text,
            });
        }
    }
}

/// The prime lending rate banks quote their best borrowers.
pub fn prime_rate_bps(world: &World) -> i32 {
    world.macro_state.policy_rate_bps + 300 + lending_premium_bps(world)
}

/// A ten-year government bond yield: expected policy rates over the decade plus a term
/// premium that rises with inflation.
pub fn ten_year_yield_bps(world: &World) -> i32 {
    let c = &world.macro_state.cycle;
    let long_run = c.neutral_real_bps + c.expected_inflation_bps;
    let now = world.macro_state.policy_rate_bps;
    (now + 3 * long_run) / 4
        + 75
        + (world.macro_state.inflation_bps - c.target_inflation_bps).max(0) / 4
}

/// Real growth over the last 52 weeks, or annualised over what history there is.
pub fn year_on_year_growth(history: &[MacroPoint]) -> i32 {
    let Some(last) = history.last() else {
        return 0;
    };
    let back = history.len().min(53) - 1;
    if back == 0 {
        return 0;
    }
    let first = history[history.len() - 1 - back];
    if first.gdp_index <= 0 {
        return 0;
    }
    let change = (last.gdp_index - first.gdp_index) as i128 * 10_000 / first.gdp_index as i128;
    (change * 52 / back as i128) as i32
}

/// Extra interest lenders charge when the economy is weak, on top of the policy rate and the
/// borrower's grade.
pub fn lending_premium_bps(world: &World) -> i32 {
    (-world.macro_state.cycle.output_gap_bps).max(0) / 4
        + crate::economy::timeline::credit_spread_bps(world)
}

/// Nominal prices outside the firms' control drift with inflation: the price index, background
/// wages (so prevailing wages follow) and the gateway import and export prices. Firms' own
/// prices follow through the market.
fn apply_nominal_drift(world: &mut World, inflation: i32) {
    let before = world.macro_state.price_index_bps.max(1);
    let after = (before + before * inflation as i64 / (52 * 10_000)).max(1);
    world.macro_state.price_index_bps = after;
    if after == before {
        return;
    }
    let scale =
        |value: i64| -> i64 { (value as i128 * after as i128 / before as i128).max(1) as i64 };
    for region in &mut world.regions {
        for wage in region.background_wage_cents.iter_mut() {
            *wage = scale(*wage);
        }
    }
    for gateway in &mut world.gateways {
        gateway.sell_price_cents = scale(gateway.sell_price_cents);
        gateway.buy_price_cents = scale(gateway.buy_price_cents);
    }
}

/// Household demand everywhere rises and falls with the output gap: a 1 percent gap moves
/// demand 1.2 percent.
fn apply_demand(world: &mut World, gap: i32) {
    let until = Tick(world.tick.0 + 2);
    for region in &mut world.regions {
        region
            .demand_modifiers
            .retain(|(cause, _, _, _)| cause != "macro_cycle");
        region
            .demand_modifiers
            .push(("macro_cycle".to_owned(), None, gap * 12 / 10, until));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn the_economy_cycles_and_the_central_bank_responds() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 11, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let mut phases = std::collections::BTreeSet::new();
        let mut rates = std::collections::BTreeSet::new();
        for _ in 0..(52 * 12) {
            weekly(&mut world, &dataset);
            world.tick = Tick(world.tick.0 + 1);
            phases.insert(world.macro_state.cycle.phase.key());
            rates.insert(world.macro_state.policy_rate_bps);
            let inflation = world.macro_state.inflation_bps;
            assert!((-300..=2_000).contains(&inflation));
            assert_eq!(
                world.macro_state.policy_rate_bps % 25,
                0,
                "rates move in quarter points"
            );
        }
        assert!(
            phases.contains("recession"),
            "twelve years hold at least one recession: {phases:?}"
        );
        assert!(
            phases.len() >= 4,
            "the cycle visits most phases: {phases:?}"
        );
        assert!(rates.len() >= 6, "the central bank moves its rate");
        assert_eq!(world.macro_state.cycle.history.len(), MACRO_HISTORY_WEEKS);
        assert!(
            world.macro_state.price_index_bps > 10_000,
            "prices drift up with inflation"
        );
    }

    #[test]
    fn the_cycle_is_deterministic() {
        let dataset = Dataset::load().unwrap();
        let run = || {
            let mut world = seed::build(&dataset, 12, ScenarioKind::Guided, ScenarioMode::Sandbox);
            for _ in 0..200 {
                weekly(&mut world, &dataset);
                world.tick = Tick(world.tick.0 + 1);
            }
            world.macro_state.cycle.history.clone()
        };
        assert_eq!(run(), run());
    }
}
