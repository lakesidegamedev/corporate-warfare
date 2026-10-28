//! The 52-week game calendar.
//!
//! A game year is exactly 52 weeks and a quarter exactly 13 weeks. This is a game calendar,
//! not an assertion that real civil years divide evenly. Week 1 of year 2000 is tick 1.

use serde::{Deserialize, Serialize};

pub const WEEKS_PER_YEAR: u32 = 52;
pub const WEEKS_PER_QUARTER: u32 = 13;
pub const START_YEAR: i32 = 2000;

/// Absolute tick index, 1-based. Tick 0 is the opening state before any week is played.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Tick(pub u32);

impl Tick {
    pub const OPENING: Tick = Tick(0);

    pub fn next(self) -> Tick {
        Tick(self.0 + 1)
    }

    /// Calendar year containing this tick.
    pub fn year(self) -> i32 {
        START_YEAR + ((self.0.saturating_sub(1)) / WEEKS_PER_YEAR) as i32
    }

    /// 1-based week within the year.
    pub fn week_of_year(self) -> u32 {
        ((self.0.saturating_sub(1)) % WEEKS_PER_YEAR) + 1
    }

    /// 1-based quarter within the year.
    pub fn quarter(self) -> u32 {
        (self.week_of_year() - 1) / WEEKS_PER_QUARTER + 1
    }

    pub fn is_quarter_boundary(self) -> bool {
        self.0 > 0 && self.week_of_year().is_multiple_of(WEEKS_PER_QUARTER)
    }

    pub fn is_year_boundary(self) -> bool {
        self.0 > 0 && self.week_of_year().is_multiple_of(WEEKS_PER_YEAR)
    }

    /// Whole weeks elapsed since an earlier tick, saturating at zero.
    pub fn weeks_since(self, earlier: Tick) -> u32 {
        self.0.saturating_sub(earlier.0)
    }

    pub fn label(self) -> String {
        self.label_from(START_YEAR)
    }

    /// Calendar year containing this tick in a scenario whose first week falls in `start_year`.
    pub fn year_from(self, start_year: i32) -> i32 {
        start_year + ((self.0.saturating_sub(1)) / WEEKS_PER_YEAR) as i32
    }

    /// Display label such as `1981 W05` for a scenario that starts in `start_year`.
    pub fn label_from(self, start_year: i32) -> String {
        format!("{} W{:02}", self.year_from(start_year), self.week_of_year())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_boundaries() {
        assert_eq!(Tick(1).year(), 2000);
        assert_eq!(Tick(1).week_of_year(), 1);
        assert_eq!(Tick(13).quarter(), 1);
        assert!(Tick(13).is_quarter_boundary());
        assert_eq!(Tick(14).quarter(), 2);
        assert_eq!(Tick(52).week_of_year(), 52);
        assert!(Tick(52).is_year_boundary());
        assert_eq!(Tick(53).year(), 2001);
        assert_eq!(Tick(53).week_of_year(), 1);
        // Five-year score lands on the last week of the fifth year.
        assert_eq!(Tick(260).year(), 2004);
        assert_eq!(Tick(260).week_of_year(), 52);
    }

    #[test]
    fn scenario_start_year_shifts_the_label() {
        assert_eq!(Tick(5).label_from(1981), "1981 W05");
        assert_eq!(Tick(53).label_from(1995), "1996 W01");
        assert_eq!(Tick(5).label(), "2000 W05");
    }

    #[test]
    fn opening_tick_has_no_quarter_boundary() {
        assert!(!Tick::OPENING.is_quarter_boundary());
        assert_eq!(Tick::OPENING.year(), 2000);
    }
}
