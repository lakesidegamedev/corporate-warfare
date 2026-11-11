//! Careers: the content table (2.0).
//!
//! `content/careers.json` holds every number of the career frame: the helm start's rank band and
//! fortune cover, the career lengths, the ambition thresholds, the board's warning and the floor
//! under which a player with no company has lost. It is not part of the dataset hash; a career
//! copies the numbers it judges by into its save state when it starts.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::Difficulty;

const CAREERS_JSON: &str = include_str!("../../../content/careers.json");

#[derive(Clone, Debug, Deserialize)]
pub struct DifficultyBps {
    pub easy: i64,
    pub standard: i64,
    pub hard: i64,
}

impl DifficultyBps {
    pub fn get(&self, difficulty: Difficulty) -> i64 {
        match difficulty {
            Difficulty::Easy => self.easy,
            Difficulty::Standard => self.standard,
            Difficulty::Hard => self.hard,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Helm {
    /// The league ranks (by company value, 1 is the largest) the seed firms are offered from.
    pub rank_from: u32,
    pub rank_to: u32,
    pub offers: u32,
    /// When the standard fortune cannot buy control, it is raised to this share of the block's
    /// price (10000 is exactly the price), then scaled by difficulty.
    pub fortune_cover_bps: i64,
    pub fortune_difficulty_bps: DifficultyBps,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Board {
    /// Weeks from the start of the career before the board judges the player.
    pub grace_weeks: u32,
    /// Four-week periods of losses in a row that draw the board's warning (the rival rule).
    pub loss_periods: u32,
    /// Weeks the warning runs before removal. A profitable four-week period lifts it.
    pub warning_weeks: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Dominant {
    pub quarters: u32,
    /// An industry counts only in a quarter in which at least this many companies (groups
    /// counted once) made its output, so a near monopoly at the start is not dominance won.
    pub min_producers: u32,
    pub bronze_bps: i64,
    pub silver_bps: i64,
    pub gold_bps: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Ranks {
    pub bronze: u32,
    pub silver: u32,
    pub gold: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Survivor {
    /// Silver: the company ranks in this top share of the league (5000 is the top half).
    pub silver_top_bps: i64,
    pub gold_top_bps: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CareerTable {
    pub schema: u32,
    pub helm: Helm,
    pub lengths_years: Vec<u32>,
    pub default_years: u32,
    /// A player with no company and less than this fortune cannot found or buy one: lost.
    pub fortune_floor_cents: i64,
    /// Weeks a founder has to found a company before having none counts.
    pub found_grace_weeks: u32,
    pub board: Board,
    pub dominant: Dominant,
    pub ranks: Ranks,
    pub survivor: Survivor,
    pub events_kept: usize,
}

impl CareerTable {
    pub fn validate(&self) -> Result<(), String> {
        if self.helm.rank_from == 0 || self.helm.rank_to < self.helm.rank_from {
            return Err("helm rank band is empty".into());
        }
        if self.helm.offers == 0 || self.helm.fortune_cover_bps < 10_000 {
            return Err("helm offers or fortune cover out of range".into());
        }
        if self.lengths_years.is_empty() || !self.lengths_years.contains(&self.default_years) {
            return Err("default career length is not one of the lengths".into());
        }
        let d = &self.dominant;
        if !(0 < d.bronze_bps && d.bronze_bps <= d.silver_bps && d.silver_bps <= d.gold_bps)
            || d.gold_bps > 10_000
            || d.quarters == 0
        {
            return Err("dominant thresholds out of order".into());
        }
        let r = &self.ranks;
        if !(r.gold >= 1 && r.gold <= r.silver && r.silver <= r.bronze) {
            return Err("rank thresholds out of order".into());
        }
        if !(0 < self.survivor.gold_top_bps
            && self.survivor.gold_top_bps <= self.survivor.silver_top_bps
            && self.survivor.silver_top_bps <= 10_000)
        {
            return Err("survivor thresholds out of order".into());
        }
        Ok(())
    }
}

/// The career table, parsed once. The file is compiled in and checked by a test.
pub fn career_table() -> &'static CareerTable {
    static TABLE: OnceLock<CareerTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let table: CareerTable =
            serde_json::from_str(CAREERS_JSON).expect("content/careers.json parses");
        table.validate().expect("content/careers.json is valid");
        table
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_career_table_parses_and_validates() {
        let table = career_table();
        assert_eq!(table.schema, 1);
        assert!(table.lengths_years.contains(&5));
        assert_eq!(table.default_years, 10);
    }
}
