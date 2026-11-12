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

/// Biggest's thresholds for one start: company league places, and optionally a top share of
/// the league that also earns bronze.
#[derive(Clone, Debug, Deserialize)]
pub struct Places {
    /// When not 0, a company in this top share of the league earns bronze (5000 = top half).
    pub bronze_top_bps: i64,
    /// Places for bronze, silver and gold (1 is first). Bronze 0 means only the top share counts.
    pub bronze: u32,
    pub silver: u32,
    pub gold: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Biggest {
    pub founder: Places,
    pub helm: Places,
}

/// Richest's growth multiples for one career length, basis points of the starting fortune.
#[derive(Clone, Debug, Deserialize)]
pub struct Multiples {
    pub years: u32,
    pub bronze_bps: i64,
    pub silver_bps: i64,
    pub gold_bps: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Richest {
    pub multiples: Vec<Multiples>,
    /// Rows for one scenario, by scenario id, where its founders grow at a different pace;
    /// a length a scenario does not list falls back to `multiples`.
    #[serde(default)]
    pub scenarios: std::collections::BTreeMap<String, Vec<Multiples>>,
}

impl Richest {
    /// The multiples for a career length in a scenario: the scenario's own row for that length
    /// when it has one, else the shared row (`for_years`).
    pub fn for_scenario_years(&self, scenario: &str, years: u32) -> &Multiples {
        self.scenarios
            .get(scenario)
            .and_then(|rows| rows.iter().find(|m| m.years == years))
            .unwrap_or_else(|| self.for_years(years))
    }

    /// The multiples for a career length (the nearest shorter row, else the first).
    pub fn for_years(&self, years: u32) -> &Multiples {
        self.multiples
            .iter()
            .filter(|m| m.years <= years)
            .max_by_key(|m| m.years)
            .unwrap_or(&self.multiples[0])
    }
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
    /// Richest: growth of the player's own fortune over the career, per length.
    pub richest: Richest,
    /// Biggest: company league places, per start.
    pub biggest: Biggest,
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
        for r in [&self.biggest.founder, &self.biggest.helm] {
            let bronze_ok = r.bronze == 0 && r.bronze_top_bps > 0 || r.bronze >= r.silver;
            if !(r.gold >= 1 && r.gold <= r.silver && bronze_ok && r.bronze_top_bps <= 10_000) {
                return Err("biggest thresholds out of order".into());
            }
        }
        if self.richest.multiples.is_empty() {
            return Err("richest has no multiples".into());
        }
        for m in self
            .richest
            .multiples
            .iter()
            .chain(self.richest.scenarios.values().flatten())
        {
            if !(10_000 < m.bronze_bps
                && m.bronze_bps <= m.silver_bps
                && m.silver_bps <= m.gold_bps)
            {
                return Err("richest multiples out of order".into());
            }
        }
        for years in &self.lengths_years {
            if !self.richest.multiples.iter().any(|m| m.years == *years) {
                return Err(format!("richest has no row for {years} years"));
            }
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
