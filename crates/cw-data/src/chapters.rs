//! Chapters and board mandates: the content tables.
//!
//! `content/mandates.json` holds every number of the mandate system (size bands, difficulty
//! multipliers, rewards). `content/scenarios/<id>/chapters.json` names a scenario's chapters:
//! era names keyed by timeline event id, and a short list of generic names for chapters in which
//! no event plays. Neither file is part of the dataset hash, and the engine copies nothing from
//! them into a save: a chapter's name and a mandate's target are fixed when the chapter opens.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::Difficulty;

const MANDATES_JSON: &str = include_str!("../../../content/mandates.json");

#[derive(Clone, Debug, Deserialize)]
pub struct Rewards {
    pub bonus_bps: i64,
    pub bonus_min_cents: i64,
    pub reputation_success_bps: i32,
    pub reputation_failure_bps: i32,
    pub reputation_floor_bps: i32,
    pub reputation_ceiling_bps: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DifficultyBps {
    pub easy: i64,
    pub standard: i64,
    pub hard: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SizeBand {
    pub name: String,
    pub max_blocks: u64,
    pub share_step_bps: i64,
    pub profit_growth_bps: i64,
    pub profit_min_step_cents: i64,
    pub cash_cover_weeks: i64,
    pub debt_equity_cut_bps: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MandateTable {
    pub schema: u32,
    pub offer_window_weeks: u32,
    pub min_weeks_of_history: usize,
    pub rewards: Rewards,
    pub difficulty_bps: DifficultyBps,
    pub size_bands: Vec<SizeBand>,
    pub debt_equity_floor_bps: i64,
    pub share_ceiling_bps: i64,
    pub share_unreachable_bps: i64,
}

impl MandateTable {
    /// The stretch multiplier of a difficulty, in basis points (10,000 is standard).
    pub fn difficulty(&self, difficulty: Difficulty) -> i64 {
        match difficulty {
            Difficulty::Easy => self.difficulty_bps.easy,
            Difficulty::Standard => self.difficulty_bps.standard,
            Difficulty::Hard => self.difficulty_bps.hard,
        }
    }

    /// The band a company of `blocks` plant blocks falls in.
    pub fn band(&self, blocks: u64) -> &SizeBand {
        self.size_bands
            .iter()
            .find(|band| blocks <= band.max_blocks)
            .unwrap_or_else(|| self.size_bands.last().expect("size bands are validated"))
    }

    fn validate(&self) -> Result<(), String> {
        if self.size_bands.is_empty() {
            return Err("mandates.json has no size bands".into());
        }
        if !self
            .size_bands
            .windows(2)
            .all(|w| w[0].max_blocks < w[1].max_blocks)
        {
            return Err("mandates.json size bands must rise in max_blocks".into());
        }
        if self.rewards.reputation_failure_bps > 0 || self.rewards.reputation_success_bps < 0 {
            return Err("mandates.json rewards have the wrong signs".into());
        }
        Ok(())
    }
}

/// The mandate table, parsed once. The file is compiled in and checked by a test, so a bad
/// file cannot reach a player.
pub fn mandate_table() -> &'static MandateTable {
    static TABLE: OnceLock<MandateTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        let table: MandateTable =
            serde_json::from_str(MANDATES_JSON).expect("content/mandates.json parses");
        table.validate().expect("content/mandates.json is valid");
        table
    })
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ChapterNames {
    pub schema: u32,
    #[serde(default)]
    pub eras: BTreeMap<String, String>,
    #[serde(default)]
    pub names: Vec<String>,
}

fn chapters_json(scenario_id: &str) -> Option<&'static str> {
    match scenario_id {
        "great-lakes-2000" => Some(include_str!(
            "../../../content/scenarios/great-lakes-2000/chapters.json"
        )),
        "texas-1981" => Some(include_str!(
            "../../../content/scenarios/texas-1981/chapters.json"
        )),
        "silicon-valley-1995" => Some(include_str!(
            "../../../content/scenarios/silicon-valley-1995/chapters.json"
        )),
        "ruhr-rhine-1991" => Some(include_str!(
            "../../../content/scenarios/ruhr-rhine-1991/chapters.json"
        )),
        _ => None,
    }
}

/// The chapter names of a scenario. A scenario without a file (the engine fixture, a custom
/// scenario) gets the neutral list.
pub fn chapter_names(scenario_id: &str) -> ChapterNames {
    let mut names = chapters_json(scenario_id)
        .and_then(|json| serde_json::from_str::<ChapterNames>(json).ok())
        .unwrap_or_default();
    if names.names.is_empty() {
        names.names = [
            "The First Year",
            "Finding Footing",
            "Steady Growth",
            "The Long Game",
        ]
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::TimelineFile;

    #[test]
    fn the_mandate_table_is_valid() {
        let table = mandate_table();
        assert_eq!(table.band(1).name, "small");
        assert_eq!(table.band(4).name, "small");
        assert_eq!(table.band(5).name, "medium");
        assert_eq!(table.band(13).name, "large");
        assert_eq!(table.band(u64::MAX).name, "large");
        assert!(table.rewards.reputation_failure_bps < 0);
    }

    #[test]
    fn every_era_names_a_real_timeline_event_and_no_text_has_a_dash() {
        for id in [
            "great-lakes-2000",
            "texas-1981",
            "silicon-valley-1995",
            "ruhr-rhine-1991",
        ] {
            let names = chapter_names(id);
            assert!(names.names.len() >= 8, "{id}");
            let json = crate::timeline::timeline_json(id).unwrap();
            let file: TimelineFile = serde_json::from_str(json).unwrap();
            for (event, era) in &names.eras {
                assert!(file.events.iter().any(|e| &e.id == event), "{id}: {event}");
                assert!(!era.contains('\u{2014}') && !era.contains('\u{2013}'));
            }
            for event in &file.events {
                assert!(
                    names.eras.contains_key(&event.id),
                    "{id}: {} has no era",
                    event.id
                );
            }
            assert!(
                names
                    .names
                    .iter()
                    .all(|n| !n.contains('\u{2014}') && !n.contains('\u{2013}'))
            );
        }
    }
}
