//! The five-year score.
//!
//! Components are each 0..100: return from personal net worth, resilience from weeks without
//! past-due liabilities, control from the controlled productive group's share of the portfolio,
//! and conduct from unresolved regulatory breaches and failed worker settlements. Total is
//! 0.60 return + 0.20 resilience + 0.10 control + 0.10 conduct. Integer rational arithmetic with
//! explicit rounding. Net worth is personal cash plus direct holdings at fair value minus
//! personal liabilities; no internal revenue or parent/sub double count.

use cw_core::calendar::Tick;
use cw_core::money::round_even_div;

use crate::state::{HolderId, Score, World};

pub fn personal_net_worth(world: &World) -> i64 {
    let mut worth = world.investor.cash_cents - world.investor.personal_loans_cents;
    for (_, firm) in world.firms.iter() {
        let qty = firm.shares.qty(HolderId::Investor);
        if qty > 0 {
            worth += qty * firm.shares.fair_cents;
        }
    }
    worth.max(0)
}

pub fn compute(world: &World) -> Score {
    let ending = personal_net_worth(world);
    let start = world.investor.starting_net_worth_cents.max(1);
    // return = clamp(50 + 25 * (ending/start - 1), 0, 100)
    let ratio_bps = round_even_div(ending as i128 * 10_000, start as i128);
    let return_component = (5_000 + (ratio_bps as i64 - 10_000) * 25 / 100).clamp(0, 10_000) as i32;

    let weeks = world.weeks_elapsed.max(1);
    let resilience_bps =
        ((weeks - world.weeks_with_past_due) as i128 * 10_000 / weeks as i128) as i32;

    let group_value = crate::economy::ownership::group_value(world);
    let portfolio_value = {
        let mut value = world.investor.cash_cents.max(0);
        for (_, firm) in world.firms.iter() {
            let qty = firm.shares.qty(HolderId::Investor);
            value += qty * firm.shares.fair_cents;
        }
        value.max(1)
    };
    let control_bps =
        (group_value as i128 * 10_000 / portfolio_value as i128).clamp(0, 10_000) as i32;

    let conduct_bps = (10_000
        - world.regulatory_breaches as i32 * 1_000
        - world.failed_worker_settlements as i32 * 500)
        .clamp(0, 10_000);

    let total_bps = return_component as i64 * 60
        + resilience_bps as i64 * 20
        + control_bps as i64 * 10
        + conduct_bps as i64 * 10;

    Score {
        week: world.tick,
        return_component: return_component / 100,
        resilience_component: resilience_bps / 100,
        control_component: control_bps / 100,
        conduct_component: conduct_bps / 100,
        total: (total_bps / 10_000) as i32,
        assisted: world.investor.assisted_score,
        personal_net_worth_cents: ending,
    }
}

/// Week 260 pauses the game for the score; sandbox continues afterwards.
pub fn should_score(world: &World) -> bool {
    world.mode == crate::state::ScenarioMode::FiveYear
        && world.tick.0 == 260
        && world.score.is_none()
}

pub fn update(world: &mut World) {
    if should_score(world) {
        let score = compute(world);
        world.score = Some(score);
        world.pending_pause = Some("five_year_score".to_owned());
    }
    let _ = Tick(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_data::Dataset;

    #[test]
    fn a_flat_career_scores_fifty() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 101, ScenarioKind::Guided, ScenarioMode::FiveYear);
        world.weeks_elapsed = 260;
        world.tick = Tick(260);
        let score = compute(&world);
        assert_eq!(score.return_component, 50);
        assert_eq!(score.resilience_component, 100);
    }

    #[test]
    fn score_is_bounded_and_weights_sum_to_one() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 102, ScenarioKind::Guided, ScenarioMode::FiveYear);
        let score = compute(&world);
        for component in [
            score.return_component,
            score.resilience_component,
            score.control_component,
            score.conduct_component,
        ] {
            assert!((0..=100).contains(&component));
        }
        assert!((0..=100).contains(&score.total));
    }
}
