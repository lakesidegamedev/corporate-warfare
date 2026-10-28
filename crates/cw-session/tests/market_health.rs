//! Market health (1.7.1): in every scenario, over eight seeds of two years, every good on the
//! Markets board has producers and buyers, and prices move without a week-on-week sawtooth.
//!
//! Run `cargo run --release -p cw-session --example market_health -- all` for the same report
//! with a table per scenario. The threshold test reads the board a player sees.

#[path = "common/market_health.rs"]
mod market_health;

use market_health::{SCENARIOS, render, run};

const SEEDS: u64 = 8;
const WEEKS: u32 = 104;
/// Direction reversals of material weekly price moves per 52 weeks that no good may exceed. The
/// sawtooth of 1.7.0 reversed every week or two (up to 26 a year); a smooth price reverses a few
/// times a year at most, when a producer opens or the world index turns.
const MAX_SWINGS_PER_YEAR: i64 = 8;
/// The mean over the board must stay well under the worst allowed.
const MAX_MEAN_SWINGS_PER_YEAR: i64 = 2;

fn check(scenario: &str) {
    let rows = run(scenario, SEEDS, WEEKS);
    println!("{}", render(scenario, SEEDS, WEEKS, &rows));
    assert!(!rows.is_empty(), "{scenario}: the board lists goods");
    for hidden in ["training", "water", "storage_space"] {
        assert!(
            !rows.contains_key(hidden),
            "{scenario}: {hidden} has no market and must not be listed"
        );
    }
    let mut dead = Vec::new();
    let mut swinging = Vec::new();
    for (id, good) in &rows {
        // A listed good is made, sold to buyers, and trades in most weeks it is listed.
        if good.made == 0 || good.sold == 0 {
            dead.push(format!(
                "{id} (made {}, sold {}, imported {}, exported {})",
                good.made, good.sold, good.imported, good.exported
            ));
        }
        if good.swings_per_year() > MAX_SWINGS_PER_YEAR {
            swinging.push(format!("{id} ({} a year)", good.swings_per_year()));
        }
    }
    assert!(dead.is_empty(), "{scenario}: dead goods: {dead:?}");
    assert!(
        swinging.is_empty(),
        "{scenario}: prices that saw-tooth: {swinging:?}"
    );
    let mean: i64 = rows.values().map(|g| g.swings_per_year()).sum::<i64>() / rows.len() as i64;
    assert!(
        mean <= MAX_MEAN_SWINGS_PER_YEAR,
        "{scenario}: mean {mean} swings a year across the board"
    );
}

#[test]
fn great_lakes_2000_markets_are_alive_and_smooth() {
    check(SCENARIOS[0]);
}

#[test]
fn texas_1981_markets_are_alive_and_smooth() {
    check(SCENARIOS[1]);
}

#[test]
fn silicon_valley_1995_markets_are_alive_and_smooth() {
    check(SCENARIOS[2]);
}

#[test]
fn ruhr_rhine_1991_markets_are_alive_and_smooth() {
    check(SCENARIOS[3]);
}
