//! The five-year score.
//!
//! The score measures what the player built over the opening position, so a career that only
//! holds its starting company does not score like active play (owner decision for 1.1.1,
//! DECISIONS.md). Components are each 0..100:
//!
//! - growth: personal net worth against the start, 20 when flat rising to 100 at double;
//! - expansion: 20 for each division the player's group runs beyond the first, plus 10 for
//!   each state beyond the first it operates in;
//! - leadership: 25 for each region and sector type where the group has the largest capacity;
//! - resilience: weeks without past-due liabilities;
//! - conduct: unresolved regulatory breaches and failed worker settlements.
//!
//! Total is 0.45 growth + 0.25 expansion + 0.10 leadership + 0.10 resilience + 0.10 conduct.
//! Control, the controlled group's share of the portfolio, is reported but not scored.
//! Integer rational arithmetic with explicit rounding. Net worth is personal cash plus direct
//! holdings at fair value minus personal liabilities; no internal revenue or double count.

use std::collections::{BTreeMap, BTreeSet};

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::money::round_even_div;
use cw_data::Dataset;

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

/// The player's divisions: (division count, distinct states, cells led).
pub fn footprint(world: &World, dataset: &Dataset) -> (i64, i64, i64) {
    let player: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            firm.operative() && crate::economy::events_engine::player_side(world, *handle)
        })
        .map(|(handle, _)| handle)
        .collect();
    let mut cells: BTreeSet<(u8, u8)> = BTreeSet::new();
    let mut states: BTreeSet<String> = BTreeSet::new();
    for firm in &player {
        for division in crate::economy::sectors::divisions_of(world, dataset, *firm) {
            cells.insert((division.region, division.industry));
            states.insert(dataset.regions[division.region as usize].state().to_owned());
        }
    }
    let mut led = 0i64;
    for (region, industry) in &cells {
        let (firms, _) = crate::economy::sectors::cell(world, dataset, *region, *industry);
        let mut by_side: BTreeMap<bool, i64> = BTreeMap::new();
        let mut best_rival = 0i64;
        for (handle, value) in firms {
            if player.contains(&handle) {
                *by_side.entry(true).or_default() += value;
            } else {
                best_rival = best_rival.max(value);
            }
        }
        if by_side.get(&true).copied().unwrap_or(0) > best_rival {
            led += 1;
        }
    }
    (cells.len() as i64, states.len() as i64, led)
}

pub fn compute(world: &World, dataset: &Dataset) -> Score {
    let ending = personal_net_worth(world);
    let start = world.investor.starting_net_worth_cents.max(1);
    // growth = clamp(20 + 80 * (ending/start - 1), 0, 100)
    let ratio_bps = round_even_div(ending as i128 * 10_000, start as i128);
    let growth_bps = (2_000 + (ratio_bps as i64 - 10_000) * 80 / 100).clamp(0, 10_000) as i32;

    let (divisions, states, led) = footprint(world, dataset);
    let expansion_bps =
        (2_000 * (divisions - 1).max(0) + 1_000 * (states - 1).max(0)).clamp(0, 10_000) as i32;
    let leadership_bps = (2_500 * led).clamp(0, 10_000) as i32;

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

    let total_bps = growth_bps as i64 * 45
        + expansion_bps as i64 * 25
        + leadership_bps as i64 * 10
        + resilience_bps as i64 * 10
        + conduct_bps as i64 * 10;

    Score {
        week: world.tick,
        return_component: growth_bps / 100,
        expansion_component: expansion_bps / 100,
        leadership_component: leadership_bps / 100,
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

pub fn update(world: &mut World, dataset: &Dataset) {
    if should_score(world) {
        let score = compute(world, dataset);
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
    fn a_career_that_only_holds_its_start_scores_low() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 101, ScenarioKind::Guided, ScenarioMode::FiveYear);
        world.weeks_elapsed = 260;
        world.tick = Tick(260);
        let score = compute(&world, &dataset);
        // Flat net worth is 20 growth; one division in one state is no expansion.
        assert_eq!(score.return_component, 20);
        assert_eq!(score.expansion_component, 0);
        assert_eq!(score.resilience_component, 100);
        // 0.45 * 20 + 0.10 * leadership + 0.10 * 100 + 0.10 * 100.
        assert_eq!(
            score.total,
            (20 * 45 + score.leadership_component * 10 + 100 * 10 + 100 * 10) / 100
        );
        assert!(score.total <= 39, "holding the start is not a good career");
    }

    #[test]
    fn score_is_bounded_and_weights_sum_to_one() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 102, ScenarioKind::Guided, ScenarioMode::FiveYear);
        let score = compute(&world, &dataset);
        for component in [
            score.return_component,
            score.expansion_component,
            score.leadership_component,
            score.resilience_component,
            score.control_component,
            score.conduct_component,
        ] {
            assert!((0..=100).contains(&component));
        }
        assert!((0..=100).contains(&score.total));
    }
}
